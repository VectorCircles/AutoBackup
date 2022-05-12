use std::{ops::Sub, pin::Pin, str::FromStr, sync::Arc};

use chrono::{DateTime, Utc};
use cron::Schedule;
use futures_timer::Delay;

/// Defines the datetime printed format
#[inline(always)]
pub fn utc_to_string(date: DateTime<Utc>) -> String {
    date.format("%F-%H-%M-%S").to_string()
}

/// Creates a delayed future, given a cron string.
///
/// Panics if the cron string is invalid.
pub async fn await_next_call(cron: impl AsRef<str>) -> Delay {
    Delay::new(
        Schedule::from_str(cron.as_ref())
            .unwrap()
            .upcoming(chrono::Utc)
            .next()
            .unwrap()
            .sub(chrono::Utc::now())
            .to_std()
            .unwrap(),
    )
}
