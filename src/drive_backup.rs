use crate::{
    config::{Config, GoogleDriveConfig},
    util::{utc_to_string, Backup, Lock},
};
use chrono::{DateTime, Utc};
use google_drive3::{
    hyper::{self, body},
    hyper_rustls, oauth2, DriveHub,
};
use indicatif::ProgressBar;
use log::*;
use std::{borrow::Borrow, collections::LinkedList, pin::Pin, sync::Arc};
use tokio::sync::Mutex;

pub struct DriveBackup {
    config: Pin<Arc<Mutex<Config>>>,
    hub: Pin<Arc<Mutex<DriveHub>>>,
}

#[async_trait::async_trait]
impl Backup for DriveBackup {
    async fn new(config: Lock<Config>) -> Self {
        trace!("Constructing DriveBackup");
        let (client_id, client_secret) = {
            let config = config.lock().await;
            let GoogleDriveConfig {
                client_id,
                client_secret,
                ..
            } = config.borrow().google_drive.as_ref().unwrap();
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
            hyper::Client::builder().build(
                hyper_rustls::HttpsConnectorBuilder::new()
                    .with_native_roots()
                    .https_or_http()
                    .enable_http1()
                    .build(),
            ),
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

    async fn backup_changes(&self) {
        trace!("Called DriveBackup::backup_changes");
        /* ---- SYSTEM STATE PROCESSING ---- */
        let update_time = {
            let update_time = self
                .config
                .lock()
                .await
                .google_drive
                .as_mut()
                .unwrap()
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
            self.config
                .lock()
                .await
                .google_drive
                .as_ref()
                .unwrap()
                .prefix,
            utc_to_string(Utc::now())
        );
        let files = self
            .hub
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
                let date = |s: &Option<String>| {
                    s.as_ref()
                        .map(AsRef::as_ref)
                        .map(DateTime::parse_from_rfc3339)
                        .map(Result::unwrap)
                };
                let modified_time = date(&file.modified_time)
                    .expect("Modified DateTime did not arrive with the response");
                let created_time = date(&file.created_time)
                    .expect("Created DateTime did not arrive with the response");

                (update_time <= modified_time || update_time <= created_time)
                    .then(|| file.id.unwrap())
                    .zip(file.name)
            })
            .collect::<Vec<_>>();

        if files.is_empty() {
            return;
        }

        let progress_bar = Arc::pin(Mutex::new(ProgressBar::new(files.len() as u64)));
        info!("Pulling drive updates");
        futures::future::join_all(files.into_iter().map(|(id, name)| {
            let current_dir = current_dir.clone();
            let progress_bar = progress_bar.clone();
            async move {
                self.download_drive_file(current_dir, id, name).await;
                progress_bar.lock().await.inc(1);
            }
        }))
        .await;
        progress_bar.lock().await.finish();
        trace!("Finished pulling dive updates");
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
                config.google_drive.as_ref().unwrap().prev_update_time
            );
            if config
                .google_drive
                .as_ref()
                .unwrap()
                .prev_update_time
                .is_none()
            {
                config.google_drive.as_mut().unwrap().prev_update_time =
                    Some(Utc::now().to_rfc3339());
                config.write();
            } else {
                debug!("No initial backup required");
                return;
            }
        }

        /* ---- LOADING INITIAL VERSION OF THE FILES ---- */
        info!("Performing initial backup of Google Drive");
        let base_directory = format!(
            "{}/base",
            self.config
                .lock()
                .await
                .google_drive
                .as_ref()
                .unwrap()
                .prefix
        );
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
            .unwrap();
        info!("The initial backup consists of {} file(s)", files.len());
        let progress_bar = Arc::pin(Mutex::new(ProgressBar::new(files.len() as u64)));
        progress_bar
            .lock()
            .await
            .set_message("Initial Backup Progress");
        futures::future::join_all(
            files
                .into_iter()
                .map(|file| {
                    (
                        file.id.as_ref().unwrap().clone(),
                        file.name.as_ref().unwrap().clone(),
                    )
                })
                .map(|(id, name)| {
                    let base_directory = base_directory.clone();
                    let progress_bar = progress_bar.clone();
                    async move {
                        self.download_drive_file(&base_directory, id.clone(), name.clone())
                            .await;
                        progress_bar.lock().await.inc(1);
                    }
                }),
        )
        .await;
        progress_bar.lock().await.finish();
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
        let dest_folder = Arc::pin(format!(
            "{}/{}",
            dest_folder.as_ref(),
            self.discover_file_parents(&file_id).await
        ));

        trace!(
            "Creating directory for file {}: `{}`",
            file_id.as_ref(),
            &*dest_folder
        );
        std::fs::create_dir_all(&*dest_folder).unwrap();
        // Downloading file
        if let Ok(x) = async {
            self.hub
                .lock()
                .await
                .files()
                .get(file_id.as_ref())
                .param("alt", "media")
                .doit()
                .await
        }
        .await
        .map(|(body, _)| body)
        .map(|body| {
            let file_name = String::from(file_name.as_ref());
            let dest_folder = dest_folder.clone();
            async move {
                std::fs::write(
                    format!("{}/{}", dest_folder, file_name),
                    body::to_bytes(body).await.unwrap(),
                )
                .unwrap();
            }
        }) {
            x.await;
            trace!("Downloaded {} ({})", file_id.as_ref(), file_name.as_ref());
        }
        // If failed to download the file -- trying to export it from Google Docs
        else if let Some(ext) = futures::future::join_all(
            [
                ("application/rtf", "rtf"),
                ("application/vnd.oasis.opendocument.text", "opendoc"),
                (
                    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                    "googledoc",
                ),
                ("application/pdf", "pdf"),
                ("application/epub+zip", "epub.zip"),
                ("application/zip", "zip"),
                ("text/html", "html"),
                ("text/plain", "txt"),
            ]
            .into_iter()
            .map(|(mime, ext)| {
                let file_id = file_id.as_ref().to_string();
                async move {
                    self.hub
                        .lock()
                        .await
                        .files()
                        .export(file_id.as_ref(), mime)
                        .doit()
                        .await
                        .map(|res| (res, ext))
                }
            }),
        )
        .await
        .into_iter()
        .find(Result::is_ok)
        .map(Result::unwrap)
        .map(|(res, ext)| {
            let file_name = file_name.as_ref().to_string();
            async move {
                std::fs::write(
                    format!("{}/{}.{}", dest_folder, file_name, ext),
                    body::to_bytes(res).await.unwrap(),
                )
                .unwrap();
                ext
            }
        }) {
            trace!(
                "Exported {} ({}) as {}",
                file_id.as_ref(),
                file_name.as_ref(),
                ext.await
            );
        } else {
            warn!(
                "Failed to download {} ({})",
                file_name.as_ref(),
                file_id.as_ref(),
            )
        }
    }

    /// Given file file id, returns its path on the drive _excluding the filename_
    async fn discover_file_parents(&self, file_id: impl AsRef<str>) -> String {
        // Iteratively getting the file's full path
        {
            let mut file_id = Some(String::from(file_id.as_ref()));
            let mut path = LinkedList::new();
            while file_id.is_some() {
                let hub = self.hub.lock().await;
                let (_, file) = hub
                    .files()
                    .get(file_id.as_ref().unwrap())
                    .param("fields", "*")
                    .doit()
                    .await
                    .unwrap();
                file_id = file.parents.and_then(|parents| parents.into_iter().next());
                path.push_front(
                    file.name
                        .unwrap_or_else(|| file_id.as_ref().unwrap().clone()),
                );
            }
            path.pop_back(); // Removing filename, as it isn't a folder
            path
        }
        // Generating the path string
        .into_iter()
        .fold(String::new(), |mut partial_path, element| {
            partial_path += "/";
            partial_path += &element;
            partial_path
        })
    }
}
