use config::{Config, Environment, File};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use std::env::var;
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

pub fn get_config(config_dir: &Path) -> Result<Settings, config::ConfigError> {
    let env = var("APP_ENVIRONMENT").unwrap_or("local".into());
    let env_filename = format!("{env}.toml");
    let settings = Config::builder()
        .add_source(File::from(config_dir.join("base.toml")))
        .add_source(File::from(config_dir.join(env_filename)))
        // allows APP_DB__PASSWD=abc to get into the conf correctly
        .add_source(
            Environment::with_prefix("APP")
                .prefix_separator("_")
                .separator("__"),
        )
        .build()?;

    settings.try_deserialize()
}
