use crate::setup::{get, post, post_no_body, spawn_app};
use reqwest::{Client, StatusCode};
use serde_json::{Value, json};
use std::error::Error;

#[tokio::test]
async fn create_a_token() -> Result<(), Box<dyn Error>> {
    let (endpoint, _) = spawn_app().await?;
    let client = Client::new();

    let tkn_req = serde_json::from_str(r#"{ "label": "test_tkn" }"#)?;

    let response = post(format!("{endpoint}/admin/tokens"), &client, &tkn_req).await?;
    let status = response.status();
    let body: Value = response.json().await?;

    assert_eq!(StatusCode::CREATED, status);
    assert_eq!("test_tkn", body["label"]);
    assert!(!body["token"].as_str().unwrap().is_empty());

    Ok(())
}

#[tokio::test]
async fn fail_revoking_non_existing_tkn() -> Result<(), Box<dyn Error>> {
    let (endpoint, _) = spawn_app().await?;
    let client = Client::new();

    let revoked = post_no_body(
        format!("{endpoint}/admin/tokens/no_test_tkn/revoke"),
        &client,
    )
    .await?
    .status();

    assert_eq!(StatusCode::NOT_FOUND, revoked);

    Ok(())
}

#[tokio::test]
async fn create_use_revoke_block_tocken() -> Result<(), Box<dyn Error>> {
    let (endpoint, _) = spawn_app().await?;
    let client = Client::new();
    let tkn_req = serde_json::from_str(r#"{ "label": "test_tkn" }"#)?;

    let tkn_res: Value = post(format!("{endpoint}/admin/tokens"), &client, &tkn_req)
        .await?
        .json()
        .await?;

    let tkn = tkn_res["token"].as_str().ok_or("No token")?;

    let allowed_access_mission: Value = client
        .get(format!("{endpoint}/missions"))
        .bearer_auth(tkn)
        .send()
        .await?
        .json()
        .await?;

    let revoked = post_no_body(format!("{endpoint}/admin/tokens/test_tkn/revoke"), &client)
        .await?
        .status();

    let rejected_req = client
        .get(format!("{endpoint}/missions"))
        .bearer_auth(tkn)
        .send()
        .await?
        .status();

    assert_eq!(json!([]), allowed_access_mission);
    assert_eq!(StatusCode::OK, revoked, "Token revoke failed");
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        rejected_req,
        "Fetch should not be allowed"
    );

    Ok(())
}

#[tokio::test]
async fn reject_un_authed_req() -> Result<(), Box<dyn Error>> {
    let (endpoint, _) = spawn_app().await?;
    let client = Client::new();

    let unauthorized = get(format!("{endpoint}/missions/some_mission"), &client)
        .await?
        .status();

    assert_eq!(StatusCode::UNAUTHORIZED, unauthorized);

    Ok(())
}
