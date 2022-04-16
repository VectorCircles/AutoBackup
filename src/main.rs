use chrono::Utc;
use log::*;
use log4rs::append::file::FileAppender;
use log4rs::config::Appender;
use log4rs::config::Root;
use log4rs::encode::pattern::PatternEncoder;
use log4rs::Config;
use std::sync::mpsc;
use std::sync::Arc;
use tokio_cron_scheduler::{Job, JobScheduler};

#[tokio::main]
async fn main() {
    // LOGGER SETUP
    log4rs::init_config(
        Config::builder()
            .appender(
                Appender::builder().build(
                    "logfile",
                    Box::new(
                        FileAppender::builder()
                            .encoder(Box::new(PatternEncoder::new("{f} -- {l} -- {m}\n")))
                            .build(format!("log/log-{}.txt", Utc::now().to_rfc2822()))
                            .unwrap(),
                    ),
                ),
            )
            .build(
                Root::builder()
                    .appender("logfile")
                    .build(LevelFilter::Trace),
            )
            .unwrap(),
    )
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
