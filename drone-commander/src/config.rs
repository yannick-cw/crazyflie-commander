use config::{Config, Environment, File};
use serde::Deserialize;
use std::path::Path;
use url::Url;

#[derive(Deserialize, Debug)]
pub struct Settings {
    pub mission_store: MissionStoreSettings,
}

#[derive(Deserialize, Debug)]
pub enum Location {
    Local,
    Remote,
}

#[derive(Deserialize, Debug)]
pub struct MissionStoreSettings {
    pub location: Location,
    pub url: Url,
}

pub fn get_config(config_dir: &Path) -> Result<Settings, config::ConfigError> {
    let settings = Config::builder()
        .add_source(File::from(config_dir.join("conf.toml")))
        // allows TUI_MISSION_STORE__LOCATION=REMOTE to get into the conf correctly
        .add_source(
            Environment::with_prefix("TUI")
                .prefix_separator("_")
                .separator("__"),
        )
        .build()?;

    settings.try_deserialize()
}
