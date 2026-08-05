use async_trait::async_trait;
use drone_control::Command;
use futures::StreamExt;
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use std::io::Error;
use std::path::Path;
use tokio::fs;
use tokio::fs::DirEntry;
use tokio_stream::wrappers::ReadDirStream;
use tracing::{error, warn};

// not send as not share between threads right now
#[async_trait(?Send)]
pub trait MissionService {
    async fn list_missions(&self) -> Vec<(String, Vec<Command>)>;
    async fn list_recordings(&self) -> Vec<(String, Vec<Command>)>;
}
// --- Http Loader
#[derive(Debug, Default, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct MissionResponse {
    pub name: String,
    pub mission: Vec<Command>,
}

pub struct HttpMission {
    pub url: Url,
}

#[async_trait(?Send)]
impl MissionService for HttpMission {
    async fn list_missions(&self) -> Vec<(String, Vec<Command>)> {
        let client = Client::new();

        let http_res: color_eyre::Result<Vec<MissionResponse>> = async {
            let uri = self.url.clone().join("missions")?;
            let res = client.get(uri).send().await?;
            let json: Vec<MissionResponse> = res.json().await?;
            Ok(json)
        }
        .await;

        http_res
            .inspect_err(|err| error!("Failed fetching missions at {} with {}", self.url, err))
            .map(|res| res.into_iter().map(|m| (m.name, m.mission)).collect())
            .unwrap_or(vec![])
    }

    async fn list_recordings(&self) -> Vec<(String, Vec<Command>)> {
        vec![]
    }
}

// --- File Loader
pub struct FileMission {
    pub file_path: String,
}

#[async_trait(?Send)]
impl MissionService for FileMission {
    async fn list_missions(&self) -> Vec<(String, Vec<Command>)> {
        read_missions(Path::new(&self.file_path)).await
    }

    async fn list_recordings(&self) -> Vec<(String, Vec<Command>)> {
        read_missions(&Path::new(&self.file_path).join("recordings")).await
    }
}

async fn read_missions(path: &Path) -> Vec<(String, Vec<Command>)> {
    match fs::read_dir(Path::new("./drone-commander").join(path)).await {
        Ok(dir) => {
            ReadDirStream::new(dir)
                .filter_map(|entry| async {
                    match read_file(&entry.ok()?).await {
                        Ok(Some(mission)) => Some(mission),
                        Ok(None) => None,
                        Err(e) => {
                            warn!("skipping: {e}");
                            None
                        }
                    }
                })
                .collect()
                .await
        }
        Err(err) => {
            warn!("Could not load any missions {err}");
            vec![]
        }
    }
}

async fn read_file(entry: &DirEntry) -> Result<Option<(String, Vec<Command>)>, Error> {
    let file_path = entry.path();
    if entry.file_type().await?.is_file() && file_path.extension() == Some("json".as_ref()) {
        let file_content = fs::read_to_string(&file_path).await?;

        let file_name = file_path.file_stem().and_then(|s| s.to_str()).unwrap();

        let mission: Vec<Command> = serde_json::from_str(&file_content)?;
        Ok(Some((file_name.to_owned(), mission)))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::init_tracing;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn fetch_missions_from_http() {
        init_tracing();
        let test_mission = vec![Command::Takeoff {
            height: Default::default(),
            duration: Default::default(),
        }];

        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/missions"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(vec![MissionResponse {
                    name: "test_mission".to_string(),
                    mission: test_mission.clone(),
                }]),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        let http = HttpMission {
            url: mock_server.uri().parse().unwrap(),
        };

        let missions = http.list_missions().await;

        assert_eq!(vec![("test_mission".to_string(), test_mission)], missions);
    }
}
