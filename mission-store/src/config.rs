use config::{Config, File};
use secrecy::{ExposeSecret, SecretString};
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
    pub passwd: SecretString,
    pub url: String,
    pub name: String,
    pub port: i16,
}

impl DBSettings {
    pub fn connection_string(&self) -> SecretString {
        format!(
            "postgres://{}:{}@{}:{}/{}",
            self.user,
            self.passwd.expose_secret(),
            self.url,
            self.port,
            self.name
        )
        .into()
    }

    pub fn connection_string_no_db(&self) -> SecretString {
        format!(
            "postgres://{}:{}@{}:{}/",
            self.user,
            self.passwd.expose_secret(),
            self.url,
            self.port
        )
        .into()
    }
}

pub fn get_config(path: &Path) -> Result<Settings, config::ConfigError> {
    let settings = Config::builder().add_source(File::from(path)).build()?;

    settings.try_deserialize()
}
