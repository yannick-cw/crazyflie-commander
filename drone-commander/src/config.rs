use color_eyre::Report;
use color_eyre::eyre::eyre;
use config::{Config, Environment, File};
use serde::Deserialize;
use std::path::Path;
use url::Url;

#[derive(Deserialize, Debug)]
pub struct Settings {
    pub mission_store: MissionStoreSettings,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum MissionStoreSettings {
    Local(LocalStoreSettings),
    Remote(RemoteStoreSettings),
}

#[derive(Deserialize, Debug)]
pub struct LocalStoreSettings {
    pub file_path: String,
}

#[derive(Deserialize, Debug)]
pub struct RemoteStoreSettings {
    pub url: Url,
    pub key: String,
}

struct ConfFile(String);

impl TryFrom<String> for ConfFile {
    type Error = Report;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "local" => Ok(ConfFile("local.toml".to_string())),
            "remote" => Ok(ConfFile("remote.toml".to_string())),
            other => Err(eyre!("Got: {other} - expecting `local` or `remote`")),
        }
    }
}

pub fn get_config(config_dir: &Path) -> color_eyre::Result<Settings> {
    let environment: ConfFile = std::env::var("TUI_CONFIG_LOCATION")
        .unwrap_or_else(|_| "local".into())
        .try_into()?;

    let mission_store = Config::builder()
        .add_source(File::from(config_dir.join(environment.0)))
        // allows TUI_REMOTE_STORE_SETTINGS__KEY=xxx to get into the conf correctly
        .add_source(
            Environment::with_prefix("TUI")
                .prefix_separator("_")
                .separator("__"),
        )
        .build()?
        .try_deserialize::<Settings>()?;

    Ok(mission_store)
}
