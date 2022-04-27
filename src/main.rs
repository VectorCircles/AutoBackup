use log::*;
use std::str::FromStr;
use std::sync::Arc;

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

    // MAIN LOOP DEFITIONS
    let config = Arc::pin(tokio::sync::Mutex::new(config::init()));
    {
        let config = config.clone();
        async move {
            // BACKUP ROUTINE
            let backup_cron = cron::Schedule::from_str(&config.lock().await.backup_cron).unwrap();
            let drive = drive_backup::DriveBackup::new(config).await;
            loop {
                futures_timer::Delay::new({
                    let wait_duration = (backup_cron.upcoming(chrono::Utc).next().unwrap()
                        - chrono::Utc::now())
                    .to_std()
                    .unwrap();
                    debug!(
                        "Awaiting for the next backup call (will trigger in {} seconds)",
                        wait_duration.as_secs()
                    );
                    wait_duration
                })
                .await;
                debug!("Backup call received");
                {
                    trace!("Calling `drive.backup_changes`");
                    drive.backup_changes().await;
                    trace!("Finished `drive.backup_changes`");
                }
            }
        }
    }
    .await;
}

pub mod config;
pub mod drive_backup;
