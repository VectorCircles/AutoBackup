use log::*;
use std::sync::mpsc;
use std::sync::Arc;
use tokio_cron_scheduler::{Job, JobScheduler};

#[tokio::main]
async fn main() {
    // LOGGER SETUP
    flexi_logger::Logger::try_with_str("info, vectorcircles_auto_backup=trace")
        .unwrap()
        .log_to_file(flexi_logger::FileSpec::default().directory("log"))
        .duplicate_to_stderr(flexi_logger::Duplicate::Debug)
        .start()
        .unwrap();

    // MAIN LOOP DEFITIONS
    let config = Arc::pin(tokio::sync::Mutex::new(config::init()));
    let (snd, rcv) = mpsc::channel::<()>();
    let snd = Arc::pin(std::sync::Mutex::new(snd));

    let backup = {
        let config = config.clone();
        async move {
            // BACKUP THREAD
            std::thread::spawn(|| async move {
                let drive = drive_backup::DriveBackup::new(config).await;
                loop {
                    debug!("Awaiting for the next backup call");
                    rcv.recv().unwrap();
                    debug!("Backup call received");
                    {
                        trace!("Calling `drive.backup_changes`");
                        drive.backup_changes().await;
                        trace!("Finished `drive.backup_changes`");
                    }
                }
            })
            .join()
            .unwrap()
            .await
        }
    };

    let scheduler_config = config;
    let scheduler = async {
        // SCHEDULER THREAD
        let config = scheduler_config;
        let mut sched = JobScheduler::new();
        sched
            .add(
                Job::new(config.lock().await.backup_cron.as_str(), move |_, _| {
                    debug!("Scheduler calls backup");
                    snd.lock().unwrap().send(()).unwrap()
                })
                .unwrap(),
            )
            .unwrap();
        sched.start().await.unwrap()
    };

    futures::join!(backup, scheduler);
}

pub mod config;
pub mod drive_backup;
