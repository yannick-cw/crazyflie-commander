use reqwest::Client;
use std::error::Error;

pub async fn spawn_app() -> Result<(String, Client), Box<dyn Error>> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    tokio::spawn(mission_store::run(listener));

    let client = reqwest::Client::new();

    Ok((format!("http://127.0.0.1:{}", port), client))
}
