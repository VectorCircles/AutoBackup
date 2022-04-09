use crate::config::{Config, GoogleDriveConfig};
use chrono::{DateTime, Utc};
use google_drive3::{
    hyper::{self, body},
    hyper_rustls, oauth2, DriveHub,
};
use log::*;
use std::{borrow::Borrow, pin::Pin, sync::Arc};
use tokio::sync::Mutex;

pub struct DriveBackup {
    config: Pin<Arc<Mutex<Config>>>,
    hub: Pin<Arc<Mutex<DriveHub>>>,
}

impl DriveBackup {
    /// Instantiates the object and performs the initial backup
    pub async fn new(config: Pin<Arc<Mutex<Config>>>) -> Self {
        trace!("Constructing DriveBackup");
        let (client_id, client_secret) = {
            let config = config.lock().await;
            let Config {
                google_drive:
                    GoogleDriveConfig {
                        client_id,
                        client_secret,
                        ..
                    },
                ..
            }: &Config = config.borrow();
            (client_id.clone(), client_secret.clone())
        };
        let secret = oauth2::ApplicationSecret {
            client_id,
            client_secret,
            auth_uri: "https://accounts.google.com/o/oauth2/auth".into(),
            token_uri: "https://oauth2.googleapis.com/token".into(),
            project_id: Some("vectorcirclesbackup".into()),
            ..Default::default()
        };
        let auth = oauth2::InstalledFlowAuthenticator::builder(
            secret,
            oauth2::InstalledFlowReturnMethod::HTTPRedirect,
        )
        .build()
        .await
        .unwrap();

        let hub = Arc::pin(Mutex::new(DriveHub::new(
            hyper::Client::builder().build(hyper_rustls::HttpsConnector::with_native_roots()),
            auth,
        )));

        hub.lock()
            .await
            .changes()
            .get_start_page_token()
            .add_scope("https://www.googleapis.com/auth/drive.readonly")
            .add_scope("https://www.googleapis.com/auth/drive.metadata.readonly")
            .doit()
            .await
            .unwrap();

        let this = Self { config, hub };
        this.initial_backup().await;
        trace!("Constructed DriveBackup");
        this
    }

    /// Lazily downloads all changes from the google drive
    pub async fn backup_changes(&self) {
        trace!("Called DriveBackup::backup_changes");
        /* ---- SYSTEM STATE PROCESSING ---- */
        let update_time = {
            let update_time = self
                .config
                .lock()
                .await
                .google_drive
                .prev_update_time
                .replace(Utc::now().to_rfc3339())
                .map(|x| DateTime::parse_from_rfc3339(&x).expect("Update time is invalid."))
                .expect("The system has not been initialized.");
            self.config.lock().await.write();
            update_time
        };

        /* ---- DOWNLOADING UPDATED FILES ---- */
        let current_dir = format!(
            "{}/{}",
            self.config.lock().await.google_drive.prefix,
            Utc::now()
        );
        futures::future::join_all(
            self.hub
                .lock()
                .await
                .files()
                .list()
                .param("fields", "*")
                .doit()
                .await
                .unwrap()
                .1
                .files
                .unwrap()
                .into_iter()
                .filter_map(|file| {
                    let modified_time = file
                        .modified_time
                        .as_ref()
                        .map(|string| DateTime::parse_from_rfc3339(string).unwrap())
                        .expect("DateTime did not arrive with the response");
                    (update_time < modified_time)
                        .then(|| file.id.unwrap())
                        .zip(file.name)
                })
                .map(|(id, name)| {
                    let current_dir = current_dir.clone();
                    async move { self.download_drive_file(current_dir, id, name).await }
                }),
        )
        .await;
        trace!("Finished DriveBackup::backup_changes");
    }
}

impl DriveBackup {
    /// Lazily performs initial drive backup
    async fn initial_backup(&self) {
        /* ---- PROCESSING SYSTEM STATE ---- */
        debug!("Checking if initial backup is required");
        {
            let mut config = self.config.lock().await;
            trace!(
                "Google drive last update time is: {:?}",
                config.google_drive.prev_update_time
            );
            if config.google_drive.prev_update_time.is_none() {
                config.google_drive.prev_update_time = Some(Utc::now().to_rfc3339());
                config.write();
            } else {
                debug!("No initial backup required");
                return;
            }
        }

        /* ---- LOADING INITIAL VERSION OF THE FILES ---- */
        info!("Performing initial backup of Google Drive");
        let base_directory = format!("{}/base", self.config.lock().await.google_drive.prefix);
        trace!("Base directory path: {}", base_directory);
        let files = self
            .hub
            .lock()
            .await
            .files()
            .list()
            .doit()
            .await
            .unwrap()
            .1
            .files
            .unwrap()
            .into_iter();
        info!("The initial backup consists of {} file(s)", files.len());
        futures::future::join_all(
            files
                .map(|file| {
                    (
                        file.id.as_ref().unwrap().clone(),
                        file.name.as_ref().unwrap().clone(),
                    )
                })
                .map(|(id, name)| {
                    let base_directory = base_directory.clone();
                    async move {
                        self.download_drive_file(&base_directory, id.clone(), name.clone())
                            .await;
                    }
                }),
        )
        .await;
        info!("Done initial backup of Google Drive");
    }

    /// Downloads a single Drive file
    ///
    /// If a file cannot be downloaded -- does nothing
    async fn download_drive_file(
        &self,
        dest_folder: impl AsRef<str>,
        file_id: impl AsRef<str>,
        file_name: impl AsRef<str>,
    ) {
        trace!("Downloading {} ({})", file_id.as_ref(), file_name.as_ref());
        let hub = self.hub.lock().await;
        std::fs::create_dir_all(dest_folder.as_ref()).unwrap();

        if let Ok(x) = hub
            .files()
            .get(file_id.as_ref())
            .param("alt", "media")
            .doit()
            .await
            .map_err(|_| {
                warn!(
                    "Failed to download {} ({})",
                    file_name.as_ref(),
                    file_id.as_ref(),
                )
            })
            .map(|(body, _)| body)
            .map(|body| {
                Box::pin(async {
                    std::fs::write(
                        format!("{}/{}", dest_folder.as_ref(), file_name.as_ref()),
                        body::to_bytes(body).await.unwrap(),
                    )
                    .unwrap();
                })
            })
        {
            x.await;
            trace!("Downloaded {} ({})", file_id.as_ref(), file_name.as_ref());
        }
    }
}
