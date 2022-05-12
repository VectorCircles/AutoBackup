use serde::{Deserialize, Serialize};

pub fn init() -> Config {
    // READING CONFIG FILE SOURCE
    std::fs::read_to_string("./config.yml")
        // IF FAILED TO READ CONFIG -- GENERATE A DEFAULT ONE AND PUT IT TO THE FILE
        .map_err(|_| {
            log::error!("Failed to read the configuration file.");
            log::info!("Generating new dummy configuration file. Please, fill it up.");
            Config::default().write();
        })
        // ELSE -- PARSE CONFIG FILE
        .and_then(|file_src| {
            serde_yaml::from_str(file_src.as_str())
                .map_err(|err| log::error!("Failed to parse config file: {}", err))
        })
        .unwrap()
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Config {
    pub backup_cron: String,
    pub google_drive: Option<GoogleDriveConfig>,
    pub trello: Option<TrelloConfig>,
}

impl Config {
    /// Writes this config to `./config.yml`
    pub fn write(&self) {
        std::fs::write("./config.yml", serde_yaml::to_string(self).unwrap()).unwrap();
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            backup_cron: "*/30 * * * * *".into(),
            google_drive: Some(Default::default()),
            trello: Some(Default::default()),
        }
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub struct GoogleDriveConfig {
    pub client_id: String,
    pub client_secret: String,
    pub prefix: String,
    pub prev_update_time: Option<String>,
}

impl Default for GoogleDriveConfig {
    fn default() -> Self {
        Self {
            client_id: "put_your_client_id_here".into(),
            client_secret: "put_your_secret_here".into(),
            prefix: "./drive".into(),
            prev_update_time: None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TrelloConfig {
    pub client_id: String,
    pub client_secret: String,
    pub prefix: String,
}

impl Default for TrelloConfig {
    fn default() -> Self {
        Self {
            client_id: "put_your_client_id_here".into(),
            client_secret: "put_your_secret_here".into(),
            prefix: "./trello".into(),
        }
    }
}
