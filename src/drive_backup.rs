use std::{str::FromStr, sync::Arc};

use crate::config::{Config, GoogleDriveConfig};
use chrono::{DateTime, Utc};
use google_drive3::{
    hyper::{self, body},
    hyper_rustls, oauth2, DriveHub,
};
use indicatif::ProgressBar;
use tokio::sync::Mutex;

pub async fn init(
    Config {
        google_drive:
            GoogleDriveConfig {
                client_id,
                client_secret,
                ..
            },
        ..
    }: &Config,
) -> DriveHub {
    let secret = oauth2::ApplicationSecret {
        client_id: client_id.clone(),
        client_secret: client_secret.clone(),
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

    let hub = DriveHub::new(
        hyper::Client::builder().build(hyper_rustls::HttpsConnector::with_native_roots()),
        auth,
    );

    hub.changes()
        .get_start_page_token()
        .add_scope("https://www.googleapis.com/auth/drive.readonly")
        .add_scope("https://www.googleapis.com/auth/drive.metadata.readonly")
        .doit()
        .await
        .unwrap();

    hub
}

pub async fn initial_backup(hub: &DriveHub, config: &mut Config) {
    // SYSTEM STATE PROCESSING
    if config.google_drive.prev_update_time.is_some() {
        return;
    } else {
        config.google_drive.prev_update_time = Some(Utc::now().to_rfc3339());
        config.write();
    }

    // ACTIONS
    let Config {
        google_drive: GoogleDriveConfig { prefix, .. },
        ..
    } = config;
    let files = hub.files().list().doit().await.unwrap().1.files.unwrap();
    let progress_bar = Arc::pin(Mutex::new(ProgressBar::new(files.len() as u64)));
    std::fs::create_dir_all(format!("{}/base", prefix)).unwrap();
    futures::future::join_all(
        files
            .into_iter()
            .map(|file| {
                (
                    file.name.as_ref().unwrap().clone(),
                    hub.files()
                        .get(file.id.as_ref().unwrap())
                        .param("alt", "media")
                        .doit(),
                )
            })
            .map(|(name, res_future)| (name, async { res_future.await.unwrap_or_default().0 }))
            .map(|(name, res_future)| {
                (name, async {
                    body::to_bytes(res_future.await).await.unwrap_or_default()
                })
            })
            .map(|(name, bytes_future)| {
                let prefix = prefix.clone();
                async move {
                    std::fs::write(format!("{}/base/{}", prefix, name), bytes_future.await)
                        .unwrap_or_default()
                }
            })
            .map(|future| async {
                future.await;
                progress_bar.lock().await.inc(1);
            }),
    )
    .await;
}

pub async fn backup_changes(hub: &DriveHub, config: &mut Config) {
    // SYSTEM STATE PROCESSING
    let update_time = match config.google_drive.prev_update_time.as_ref() {
        Some(time_str) => DateTime::<Utc>::from_str(time_str).expect("DateTime string is invalid."),
        None => return,
    };
    *config.google_drive.prev_update_time.as_mut().unwrap() = Utc::now().to_rfc3339();
    config.write();

    // ACTIONS
    let Config {
        google_drive: GoogleDriveConfig { prefix, .. },
        ..
    } = config;
    futures::future::join_all(
        hub.files()
            // GETTING FILES
            .list()
            .param("fields", "*")
            .doit()
            .await
            .unwrap()
            .1
            .files
            .unwrap()
            .into_iter()
            // FILTERING OUT UNCHANGED FILES
            .filter_map(|file| {
                file.modified_time
                    .as_ref()
                    .map(|modified_date_str| {
                        DateTime::parse_from_rfc3339(modified_date_str).unwrap()
                    })
                    .map(|modified_date| update_time < modified_date)
                    .unwrap()
                    .then(|| file.id.unwrap())
                    .zip(file.name)
            })
            // REQUESTING CHANGED FILE CONTENTS AND WRITING THEM TO FS
            .map(|(file_id, file_name)| {
                let prefix = prefix.clone();
                async move {
                    download_drive_file(
                        hub,
                        format!("{}/{}", prefix, &update_time.to_rfc2822()),
                        &file_id,
                        &file_name,
                    )
                    .await
                }
            }),
    )
    .await;
}

async fn download_drive_file(
    hub: &DriveHub,
    dest_folder: impl AsRef<str>,
    file_id: impl AsRef<str>,
    file_name: impl AsRef<str>,
) {
    std::fs::create_dir_all(dest_folder.as_ref()).unwrap();
    hub.files()
        .get(file_id.as_ref())
        .param("alt", "media")
        .doit()
        .await
        .map(|(body, _)| body)
        .map(|body| async move {
            std::fs::write(
                format!("{}/{}", dest_folder.as_ref(), file_name.as_ref()),
                body::to_bytes(body).await.unwrap(),
            )
        })
        .unwrap()
        .await
        .unwrap();
}
