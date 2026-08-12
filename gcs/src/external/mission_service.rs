use crate::pages::manual_control::SetpointRecording;
use async_trait::async_trait;
use datalink::domain_types::Meters;
use futures::StreamExt;
use mission_computer::MissionItem;
use reqwest::{Client, StatusCode, Url};
use serde::{Deserialize, Serialize};
use std::io::Error;
use std::path::Path;
use std::time::Duration;
use tokio::fs;
use tokio::fs::DirEntry;
use tokio_stream::wrappers::ReadDirStream;
use tracing::{error, info, warn};

// not send as not share between threads right now
#[async_trait(?Send)]
pub trait MissionService {
    async fn list_missions(&self) -> Vec<(String, Vec<MissionItem>)>;
    async fn list_recordings(&self) -> Vec<(String, Vec<MissionItem>)>;
    async fn store_recoding(&self, recording: Vec<SetpointRecording>);
}
// --- Http Loader
#[derive(Debug, Default, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct MissionResponse {
    pub name: String,
    pub mission: Vec<MissionItem>,
}

pub struct HttpMission {
    pub url: Url,
    pub secret: String,
}

#[async_trait(?Send)]
impl MissionService for HttpMission {
    async fn list_missions(&self) -> Vec<(String, Vec<MissionItem>)> {
        let client = Client::new();

        let http_res: color_eyre::Result<Vec<MissionResponse>> = async {
            let uri = self.url.clone().join("missions")?;
            let res = client.get(uri).bearer_auth(&self.secret).send().await?;
            let json: Vec<MissionResponse> = res.json().await?;
            Ok(json)
        }
        .await;

        http_res
            .inspect_err(|err| error!("Failed fetching missions at {} with {}", self.url, err))
            .map(|res| res.into_iter().map(|m| (m.name, m.mission)).collect())
            .unwrap_or(vec![])
    }

    async fn list_recordings(&self) -> Vec<(String, Vec<MissionItem>)> {
        vec![]
    }

    async fn store_recoding(&self, recording: Vec<SetpointRecording>) {
        if let Some((name, mission)) = recording_to_mission(recording) {
            let client = Client::new();

            let http_res: color_eyre::Result<StatusCode> = async {
                let uri = self.url.clone().join("missions/")?.join(&name)?;
                info!("{uri}");
                let res = client
                    .post(uri)
                    .bearer_auth(&self.secret)
                    .json(&mission)
                    .send()
                    .await?;
                Ok(res.status())
            }
            .await;

            match http_res {
                Ok(code) if code == StatusCode::CREATED => info!("Stored recorded flight {}", name),
                Ok(other_code) => {
                    error!("Could not store flight {} - got code {}", name, other_code)
                }
                Err(err) => error!("Failed storing mission at {} with {}", self.url, err),
            }
        }
    }
}

// --- File Loader
pub struct FileMission {
    pub file_path: String,
}

#[async_trait(?Send)]
impl MissionService for FileMission {
    async fn list_missions(&self) -> Vec<(String, Vec<MissionItem>)> {
        read_missions(Path::new(&self.file_path)).await
    }

    async fn list_recordings(&self) -> Vec<(String, Vec<MissionItem>)> {
        read_missions(&Path::new(&self.file_path).join("recordings")).await
    }

    async fn store_recoding(&self, recording: Vec<SetpointRecording>) {
        if let Some((name, mission)) = recording_to_mission(recording) {
            match fs::write(
                format!("./gcs/{}/recordings/flight-{}.json", &self.file_path, name),
                serde_json::to_string(&mission).unwrap(),
            )
            .await
            {
                Ok(_) => info!("stored new recording"),
                Err(err) => warn!("could not safe recording {err}"),
            }
        }
    }
}

async fn read_missions(path: &Path) -> Vec<(String, Vec<MissionItem>)> {
    match fs::read_dir(Path::new("./gcs").join(path)).await {
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

async fn read_file(entry: &DirEntry) -> Result<Option<(String, Vec<MissionItem>)>, Error> {
    let file_path = entry.path();
    if entry.file_type().await?.is_file() && file_path.extension() == Some("json".as_ref()) {
        let file_content = fs::read_to_string(&file_path).await?;

        let file_name = file_path.file_stem().and_then(|s| s.to_str()).unwrap();

        let mission: Vec<MissionItem> = serde_json::from_str(&file_content)?;
        Ok(Some((file_name.to_owned(), mission)))
    } else {
        Ok(None)
    }
}

// util -----------------------------------------------
fn recording_to_mission(recording: Vec<SetpointRecording>) -> Option<(String, Vec<MissionItem>)> {
    let first_p = recording.first()?;
    let z = recording.last().map(|p| p.z.0).unwrap_or(2.0);
    // z=1m => 2s, z=0.5m => 1s
    let land_duration = Duration::from_secs_f32((z.max(0.0) / 0.5).min(3.0));

    let mission = vec![
        MissionItem::Takeoff {
            height: Meters(0.5),
            duration: Duration::from_secs(1),
        },
        MissionItem::MoveToWaypoint {
            x: first_p.x,
            y: first_p.y,
            z: first_p.z,
            duration: Duration::from_secs(2),
        },
        MissionItem::Setpoints {
            points: recording.iter().map(|p| p.to_setpoint()).collect(),
        },
        MissionItem::Land {
            duration: land_duration,
        },
    ];

    let mission_name = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
    Some((mission_name, mission))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::init_tracing;
    use wiremock::matchers::{bearer_token, method, path, path_regex};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn fetch_missions_from_http() {
        init_tracing();
        let test_mission = vec![MissionItem::Takeoff {
            height: Default::default(),
            duration: Default::default(),
        }];

        let mock_server = MockServer::start().await;
        let token = "test_secret";
        Mock::given(method("GET"))
            .and(path("/missions"))
            .and(bearer_token(token))
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
            secret: token.to_string(),
        };

        let missions = http.list_missions().await;

        assert_eq!(vec![("test_mission".to_string(), test_mission)], missions);
    }

    #[tokio::test]
    async fn store_recording_http() {
        init_tracing();
        let test_recording = vec![SetpointRecording {
            x: Default::default(),
            y: Default::default(),
            z: Default::default(),
            yaw_degrees: 0.0,
        }];

        let mock_server = MockServer::start().await;
        let token = "test_secret";
        Mock::given(method("POST"))
            .and(path_regex(r"^/missions/.+$"))
            .and(bearer_token(token))
            .respond_with(ResponseTemplate::new(201))
            .expect(1)
            .mount(&mock_server)
            .await;

        let http = HttpMission {
            url: mock_server.uri().parse().unwrap(),
            secret: token.to_string(),
        };

        http.store_recoding(test_recording).await;
    }
}
