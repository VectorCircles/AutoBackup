use serde::{Deserialize, Serialize};

pub fn init() -> Config {
    let src = std::fs::read_to_string("./config.yml")
        .map_err(|_| {
            std::fs::write("./config.yml", include_str!("config-example.yml")).unwrap();
        })
        .expect(
            "No config provided. The dummy configuration file was generated. Please, fill it up.",
        );
    serde_yaml::from_str(src.as_str()).unwrap()
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Config {
    pub backup_cron: String,
    pub google_drive: GoogleDriveConfig,
}

impl Config {
    /// Writes this config to `./config.yml`
    pub fn write(&self) {
        std::fs::write("./config.yml", serde_yaml::to_string(self).unwrap()).unwrap();
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub struct GoogleDriveConfig {
    pub client_id: String,
    pub client_secret: String,
    pub prefix: String,
    pub prev_update_time: Option<String>,
}
