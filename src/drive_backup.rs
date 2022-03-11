use std::sync::Arc;

use crate::config::{Config, GoogleDriveConfig};
use futures::lock::Mutex;
use google_drive3::{
    hyper::{self, body},
    hyper_rustls, oauth2, DriveHub,
};
use indicatif::ProgressBar;

pub async fn init(
    Config {
        google_drive:
            GoogleDriveConfig {
                client_id,
                client_secret,
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

pub async fn backup(hub: &DriveHub) {
    let files = hub.files().list().doit().await.unwrap().1.files.unwrap();
    let progress_bar = Arc::pin(Mutex::new(ProgressBar::new(files.len() as u64)));

    std::fs::create_dir_all("./current-backup/").unwrap();
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
            .map(|(name, res_future)| {
                (
                    name,
                    Box::pin(async { res_future.await.unwrap_or_default().0 }),
                )
            })
            .map(|(name, res_future)| {
                (
                    name,
                    Box::pin(async { body::to_bytes(res_future.await).await.unwrap_or_default() }),
                )
            })
            .map(|(name, bytes_future)| {
                Box::pin(async move {
                    std::fs::write(format!("./current-backup/{}", name), bytes_future.await)
                        .unwrap_or_default()
                })
            })
            .map(|future| async {
                future.await;
                progress_bar.lock().await.inc(1);
            }),
    )
    .await;
}
