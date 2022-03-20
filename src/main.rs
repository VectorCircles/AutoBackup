use std::sync::mpsc;
use std::sync::Arc;
use tokio_cron_scheduler::{Job, JobScheduler};

#[tokio::main]
async fn main() {
    let config = Arc::pin(tokio::sync::Mutex::new(config::init()));
    let (snd, rcv) = mpsc::channel::<()>();
    let snd = Arc::pin(std::sync::Mutex::new(snd));

    let backup_config = config.clone();
    let backup = async move {
        // BACKUP THREAD
        let config = backup_config;
        std::thread::spawn(|| async move {
            let google_drive = drive_backup::init(&*config.lock().await).await;
            drive_backup::initial_backup(&google_drive, &mut *config.lock().await).await;
            loop {
                rcv.recv().unwrap();
                drive_backup::backup_changes(&google_drive, &mut *config.lock().await).await;
            }
        })
        .join()
        .unwrap()
        .await
    };

    let scheduler_config = config;
    let scheduler = async {
        // SCHEDULER THREAD
        let config = scheduler_config;
        let mut sched = JobScheduler::new();
        sched
            .add(
                Job::new(config.lock().await.backup_cron.as_str(), move |_, _| {
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
