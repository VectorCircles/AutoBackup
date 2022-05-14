use chrono::Utc;
use tokio::sync::Mutex;

use crate::{
    config::Config,
    util::{utc_to_string, Backup, Lock},
};
use std::{pin::Pin, sync::Arc};

pub struct TrelloBackup {
    config: Lock<Config>,
}

static EXPORT_PARAMETERS: &str = "fields=all\
&actions=all\
&action_fields=all\
&actions_limit=1000\
&cards=all\
&card_fields=all\
&card_attachments=true\
&labels=all\
&lists=all\
&list_fields=all\
&members=all\
&member_fields=all\
&checklists=all\
&checklist_fields=all\
&organization=false
";

#[async_trait::async_trait]
impl Backup for TrelloBackup {
    async fn new(config: Pin<Arc<Mutex<Config>>>) -> Self {
        Self { config }
    }

    async fn backup_changes(&self) {
        // DESTRUCTURING CONFIG
        let (api_key, token, boards, prefix) = self
            .config
            .lock()
            .await
            .trello
            .as_ref()
            .map(|conf| {
                (
                    conf.api_key.clone(),
                    conf.personal_token.clone(),
                    conf.board_ids.clone(),
                    conf.prefix.clone(),
                )
            })
            .unwrap();

        // CREATING DIRECTORY
        let path = format!("{}/{}", prefix, utc_to_string(Utc::now()));
        std::fs::create_dir_all(&path).unwrap();

        // DOWNLOADING BOARDS
        futures::future::join_all(
            boards
                .into_iter()
                .map(|board_id| {
                    (
                        board_id.clone(),
                        path.clone(),
                        reqwest::get(format!(
                            "https://api.trello.com/1/boards/{}?key={}&token={}&{}",
                            board_id, api_key, token, EXPORT_PARAMETERS,
                        )),
                    )
                })
                .map(|(board_id, path, res)| async move {
                    let res = res.await.unwrap();
                    if !res.status().is_success() {
                        log::error!(
                            "CRITICAL: Trello backup has returned status code {}",
                            res.status()
                        );
                        log::error!("{:#?}", res);
                        panic!("Failed to perform Trello board backup. Details are written to log.")
                    }
                    std::fs::write(format!("{}/{}", path, board_id), res.bytes().await.unwrap())
                        .unwrap();
                }),
        )
        .await;
    }
}
