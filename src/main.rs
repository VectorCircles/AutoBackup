#[tokio::main]
async fn main() {
    let config = config::init();
    drive_backup::backup(&drive_backup::init(&config).await).await
}

pub mod drive_backup;
pub mod config {
    use serde::{Deserialize, Serialize};

    pub fn init() -> Config {
        let src = std::fs::read_to_string("./config.yml")
            .map_err(|_| {
                std::fs::write("./config.yml", include_str!("config-example.yml")).unwrap();
            })
            .expect("No config provided. The dummy configuration file was generated. Please, fill it up.");
        serde_yaml::from_str(src.as_str()).unwrap()
    }

    #[derive(Deserialize, Serialize, Debug)]
    pub struct Config {
        pub google_drive: GoogleDriveConfig,
    }

    #[derive(Deserialize, Serialize, Debug)]
    pub struct GoogleDriveConfig {
        pub client_id: String,
        pub client_secret: String,
    }
}
