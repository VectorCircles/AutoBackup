use log::*;
use std::sync::Arc;

use crate::util::await_next_call;

#[tokio::main]
async fn main() {
    // LOGGER SETUP
    flexi_logger::Logger::try_with_str("info, vectorcircles_auto_backup=trace")
        .unwrap()
        .log_to_file(
            flexi_logger::FileSpec::default()
                .directory("log")
                .basename(sys_info::hostname().unwrap_or_else(|_| String::from("unknown"))),
        )
        .duplicate_to_stdout(flexi_logger::Duplicate::Info)
        .start()
        .unwrap();

    /* ---- ROUTINE DEFINITIONS ---- */
    let config = Arc::pin(tokio::sync::Mutex::new(config::init()));
    // DRIVE BACKUP ROUTINE
    let drive_routine = {
        let config = config.clone();
        async move {
            if config.lock().await.google_drive.is_some() {
                let drive = drive_backup::DriveBackup::new(config.clone()).await;
                let cron = config.lock().await.backup_cron.clone();
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

    futures::join!(drive_routine);
}

pub mod config;
pub mod drive_backup;
pub mod util;
