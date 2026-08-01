use config::{Config, File};
use serde::Deserialize;
use std::path::Path;

#[derive(Deserialize, Debug)]
pub struct Settings {
    pub db: DBSettings,
}

#[derive(Deserialize, Debug)]
pub struct DBSettings {
    pub connection_string: String,
}

pub fn get_config(path: &Path) -> Result<Settings, config::ConfigError> {
    let settings = Config::builder()
        .add_source(File::from(path))
        .set_override_option("db.connection_string", std::env::var("DATABASE_URL").ok())?
        .build()?;

    settings.try_deserialize()
}
