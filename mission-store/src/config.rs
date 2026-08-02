use config::{Config, File};
use serde::Deserialize;
use std::path::Path;

#[derive(Deserialize, Debug)]
pub struct Settings {
    pub db: DBSettings,
    pub log_settings: LogSettings,
}

#[derive(Deserialize, Debug)]
pub struct LogSettings {
    pub log_filter: String,
    pub log_structured: bool,
}

#[derive(Deserialize, Debug)]
pub struct DBSettings {
    pub user: String,
    pub passwd: String,
    pub url: String,
    pub name: String,
    pub port: i16,
}

impl DBSettings {
    pub fn connection_string(&self) -> String {
        format!(
            "postgres://{}:{}@{}:{}/{}",
            self.user, self.passwd, self.url, self.port, self.name
        )
    }

    pub fn connection_string_no_db(&self) -> String {
        format!(
            "postgres://{}:{}@{}:{}/",
            self.user, self.passwd, self.url, self.port
        )
    }
}

pub fn get_config(path: &Path) -> Result<Settings, config::ConfigError> {
    let settings = Config::builder().add_source(File::from(path)).build()?;

    settings.try_deserialize()
}
