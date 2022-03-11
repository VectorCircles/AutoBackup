use std::sync::mpsc;
use std::sync::Arc;
use std::sync::Mutex;
use tokio_cron_scheduler::{Job, JobScheduler};

#[tokio::main]
async fn main() {
    let config = Arc::pin(config::init());
    let (snd, rcv) = mpsc::channel::<()>();
    let snd = Arc::new(Mutex::new(snd));

    let backup = async {
        // BACKUP THREAD
        let config = config.clone();
        std::thread::spawn(|| async move {
            let google_drive = drive_backup::init(&config).await;
            loop {
                rcv.recv().unwrap();
                drive_backup::backup(&google_drive).await;
            }
        })
        .join()
        .unwrap()
        .await
    };

    let scheduler = async {
        // SCHEDULER THREAD
        let mut sched = JobScheduler::new();
        sched
            .add(
                Job::new(config.backup_cron.as_str(), move |_, _| {
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
