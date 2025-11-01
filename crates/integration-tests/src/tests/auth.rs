use crate::client::TestClient;
use anyhow::Result;
use tracing::info;

pub async fn run_tests(client: &TestClient) -> Result<()> {
    test_invalid_token(client).await?;
    test_valid_token(client).await?;
    test_missing_token(client).await?;
    Ok(())
}

async fn test_invalid_token(client: &TestClient) -> Result<()> {
    info!("TEST: Invalid token should be rejected");
    
    let response = client.hub_request(
        reqwest::Method::GET,
        &format!("/{}/status", client.hub_namespace()),
        Some("invalid-token"),
    ).await?;

    assert_eq!(response.status(), 401, "Invalid token should return 401");
    info!("  ✅ Invalid token rejected");
    Ok(())
}

async fn test_valid_token(client: &TestClient) -> Result<()> {
    info!("TEST: Valid token should be accepted");
    
    let token = client.generate_test_token("test-user")?;
    
    let response = client.hub_request(
        reqwest::Method::GET,
        &format!("/{}/status", client.hub_namespace()),
        Some(&token),
    ).await?;

    assert!(
        response.status().is_success() || response.status() == 404,
        "Valid token should not return 401"
    );
    info!("  ✅ Valid token accepted");
    Ok(())
}

async fn test_missing_token(client: &TestClient) -> Result<()> {
    info!("TEST: Missing token should be rejected");
    
    let response = client.hub_request(
        reqwest::Method::GET,
        &format!("/{}/status", client.hub_namespace()),
        None,
    ).await?;

    assert_eq!(response.status(), 401, "Missing token should return 401");
    info!("  ✅ Missing token rejected");
    Ok(())
}