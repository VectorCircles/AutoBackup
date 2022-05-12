use tokio::sync::Mutex;

use crate::config::Config;
use std::{pin::Pin, sync::Arc};

pub struct TrelloBackup {
    config: Pin<Arc<Mutex<Config>>>,
}

impl TrelloBackup {
    pub async fn new(config: Pin<Arc<Mutex<Config>>>) -> Self {
        Self { config }
    }

    pub async fn backup_changes(&self) {
        todo!()
    }
}
