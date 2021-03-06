use flexi_logger::detailed_format;
use log::*;
use std::sync::Arc;

use crate::util::{await_next_call, Backup};

#[tokio::main]
async fn main() {
    let config = Arc::pin(tokio::sync::Mutex::new(config::init()));

    // LOGGER SETUP
    flexi_logger::Logger::try_with_str("info, vectorcircles_auto_backup=trace")
        .unwrap()
        .format_for_files(detailed_format)
        .log_to_file(
            flexi_logger::FileSpec::default()
                .directory("log")
                .basename(sys_info::hostname().unwrap_or_else(|_| String::from("unknown"))),
        )
        .duplicate_to_stdout(config.lock().await.cmd_log_level.into())
        .start()
        .unwrap();

    /* ---- ROUTINE DEFINITIONS ---- */
    // DRIVE BACKUP ROUTINE
    let drive_routine = {
        let config = config.clone();
        async move {
            if config.lock().await.google_drive.is_some() {
                let drive = drive_backup::DriveBackup::new(config.clone()).await;
                let cron = config
                    .lock()
                    .await
                    .google_drive
                    .as_ref()
                    .unwrap()
                    .backup_cron
                    .clone();
                loop {
                    debug!("Awaiting for the next backup call.");
                    await_next_call(&cron).await;
                    debug!("Backup call received");
                    {
                        trace!("Calling `drive.backup_changes`");
                        drive.backup_changes().await;
                        trace!("Finished `drive.backup_changes`");
                    }
                }
            }
        }
    };
    // TRELLO BACKUP ROUTINE
    let trello_routine = {
        let config = config.clone();
        async move {
            if config.lock().await.trello.is_some() {
                let trello = trello_backup::TrelloBackup::new(config.clone()).await;
                let cron = config
                    .lock()
                    .await
                    .trello
                    .as_ref()
                    .unwrap()
                    .backup_cron
                    .clone();
                loop {
                    debug!("Awaiting for the next trello backup call.");
                    await_next_call(&cron).await;
                    debug!("Backup call received");
                    {
                        trace!("Calling `trello.backup_changes`");
                        trello.backup_changes().await;
                        trace!("Finished `trello.backup_changes`");
                    }
                }
            }
        }
    };

    /* ---- LAUNCHING ROUTINES ---- */
    futures::join!(drive_routine, trello_routine);
}

pub mod config;
pub mod drive_backup;
pub mod trello_backup;
pub mod util;
