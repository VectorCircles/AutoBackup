use chrono::{DateTime, Utc};

/// Defines the datetime printed format
pub fn utc_to_string(date: DateTime<Utc>) -> String {
    date.format("%F-%H-%M-%S").to_string()
}
