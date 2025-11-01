use crate::client::TestClient;
use anyhow::Result;
use tracing::info;

pub async fn run_tests(client: &TestClient) -> Result<()> {
    test_gc_respects_active_pods(client).await?;
    info!("  (Note: Full GC tests require longer timeouts and are optional)");
    Ok(())
}

async fn test_gc_respects_active_pods(client: &TestClient) -> Result<()> {
    info!("TEST: GC should not delete recently active pods");
    
    let user_id = format!("gc-active-{}", uuid::Uuid::new_v4());
    let token = client.generate_test_token(&user_id)?;
    
    // Create pod with activity
    client.hub_request(
        reqwest::Method::GET,
        &format!("/{}/test", client.hub_namespace()),
        Some(&token),
    ).await?;

    client.wait_for_pod_running(&user_id, std::time::Duration::from_secs(60)).await?;
    
    // Keep pod active
    client.proxy_to_workshop(&user_id, "/").await?;
    
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    
    // Pod should still exist
    let pod = client.get_workshop_pod(&user_id).await?;
    assert!(pod.is_some(), "Active pod should not be deleted");
    
    info!("  ✅ GC respects active pods");
    Ok(())
}

// crates/integration-tests/src/tests/stress.rs

