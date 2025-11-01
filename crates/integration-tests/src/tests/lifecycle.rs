use crate::client::TestClient;
use anyhow::Result;
use std::time::Duration;
use tracing::info;

pub async fn run_tests(client: &TestClient) -> Result<()> {
    test_pod_creation(client).await?;
    test_pod_idempotency(client).await?;
    test_service_creation(client).await?;
    Ok(())
}

async fn test_pod_creation(client: &TestClient) -> Result<()> {
    info!("TEST: Pod should be created for new user");
    
    let user_id = format!("lifecycle-user-{}", uuid::Uuid::new_v4());
    
    // Initially no pod
    let pod = client.get_workshop_pod(&user_id).await?;
    assert!(pod.is_none(), "Pod should not exist initially");
    
    // Trigger pod creation via hub request
    let token = client.generate_test_token(&user_id)?;
    let response = client.hub_request(
        reqwest::Method::GET,
        &format!("/{}/test", client.hub_namespace()),
        Some(&token),
    ).await?;

    // Pod should be created (might take a moment)
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    let pod = client.wait_for_pod_running(&user_id, Duration::from_secs(60)).await?;
    info!("  ✅ Pod created: {}", pod.metadata.name.unwrap());
    
    Ok(())
}

async fn test_pod_idempotency(client: &TestClient) -> Result<()> {
    info!("TEST: Subsequent requests should reuse existing pod");
    
    let user_id = format!("idempotent-user-{}", uuid::Uuid::new_v4());
    let token = client.generate_test_token(&user_id)?;
    
    // First request creates pod
    client.hub_request(
        reqwest::Method::GET,
        &format!("/{}/test", client.hub_namespace()),
        Some(&token),
    ).await?;

    let pod1 = client.wait_for_pod_running(&user_id, Duration::from_secs(60)).await?;
    let pod1_name = pod1.metadata.name.clone().unwrap();
    
    // Second request should reuse same pod
    client.hub_request(
        reqwest::Method::GET,
        &format!("/{}/test", client.hub_namespace()),
        Some(&token),
    ).await?;

    let pod2 = client.get_workshop_pod(&user_id).await?.unwrap();
    let pod2_name = pod2.metadata.name.clone().unwrap();
    
    assert_eq!(pod1_name, pod2_name, "Should reuse same pod");
    info!("  ✅ Pod reused: {}", pod1_name);
    
    Ok(())
}

async fn test_service_creation(client: &TestClient) -> Result<()> {
    info!("TEST: Service should be created with pod");
    
    let user_id = format!("service-user-{}", uuid::Uuid::new_v4());
    let token = client.generate_test_token(&user_id)?;
    
    // Trigger pod creation
    client.hub_request(
        reqwest::Method::GET,
        &format!("/{}/test", client.hub_namespace()),
        Some(&token),
    ).await?;

    client.wait_for_pod_running(&user_id, Duration::from_secs(60)).await?;
    
    // Service should exist
    let service = client.get_workshop_service(&user_id).await?;
    assert!(service.is_some(), "Service should be created");
    
    info!("  ✅ Service created: {}", service.unwrap().metadata.name.unwrap());
    Ok(())
}