use tokio::sync::Mutex;

use crate::{
    config::Config,
    util::{Backup, Lock},
};
use std::{pin::Pin, sync::Arc};

pub struct TrelloBackup {
    config: Lock<Config>,
}

#[async_trait::async_trait]
impl Backup for TrelloBackup {
    async fn new(config: Pin<Arc<Mutex<Config>>>) -> Self {
        Self { config }
    }

    async fn backup_changes(&self) {
        todo!()
    }
}
