use std::{ops::Sub, pin::Pin, str::FromStr, sync::Arc};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use cron::Schedule;
use futures_timer::Delay;
use tokio::sync::Mutex;

use crate::config::Config;

/// Defines the datetime printed format
#[inline(always)]
pub fn utc_to_string(date: DateTime<Utc>) -> String {
    date.format("%F-%H-%M-%S").to_string()
}

/// Creates a delayed future, given a cron string.
///
/// Panics if the cron string is invalid.
pub async fn await_next_call(cron: impl AsRef<str>) {
    let del = Schedule::from_str(cron.as_ref())
        .unwrap()
        .upcoming(chrono::Utc)
        .next()
        .unwrap()
        .sub(chrono::Utc::now())
        .to_std()
        .unwrap();

    println!("{:?}", del.as_secs_f32());
    Delay::new(del).await
}

pub type Lock<T> = Pin<Arc<Mutex<T>>>;

#[async_trait]
pub trait Backup {
    /// Constructs Backup object, given config
    async fn new(config: Lock<Config>) -> Self;

    /// Backs the corresponding changes up
    async fn backup_changes(&self);
}
