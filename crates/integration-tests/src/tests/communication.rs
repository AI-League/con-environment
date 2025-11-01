use crate::client::TestClient;
use anyhow::Result;
use std::time::Duration;
use tracing::info;

pub async fn run_tests(client: &TestClient) -> Result<()> {
    test_health_endpoint(client).await?;
    test_proxy_communication(client).await?;
    test_idle_tracking(client).await?;
    Ok(())
}

async fn test_health_endpoint(client: &TestClient) -> Result<()> {
    info!("TEST: Health endpoint should be accessible");
    
    let user_id = format!("health-user-{}", uuid::Uuid::new_v4());
    let token = client.generate_test_token(&user_id)?;
    
    // Create pod
    client.hub_request(
        reqwest::Method::GET,
        &format!("/{}/test", client.hub_namespace()),
        Some(&token),
    ).await?;

    client.wait_for_pod_running(&user_id, Duration::from_secs(60)).await?;
    
    // Give sidecar time to start
    tokio::time::sleep(Duration::from_secs(3)).await;
    
    // Check health
    let health = client.check_workshop_health(&user_id).await?;
    
    assert_eq!(health["status"], "ok", "Health status should be ok");
    assert!(health["idle_seconds"].is_number(), "Should have idle_seconds");
    
    info!("  ✅ Health endpoint accessible");
    info!("     Idle seconds: {}", health["idle_seconds"]);
    
    Ok(())
}

async fn test_proxy_communication(client: &TestClient) -> Result<()> {
    info!("TEST: Proxy should forward to workshop container");
    
    let user_id = format!("proxy-user-{}", uuid::Uuid::new_v4());
    let token = client.generate_test_token(&user_id)?;
    
    // Create pod
    client.hub_request(
        reqwest::Method::GET,
        &format!("/{}/test", client.hub_namespace()),
        Some(&token),
    ).await?;

    client.wait_for_pod_running(&user_id, Duration::from_secs(60)).await?;
    tokio::time::sleep(Duration::from_secs(3)).await;
    
    // Send request through proxy
    let response = client.proxy_to_workshop(&user_id, "/").await?;
    
    assert_eq!(response.status(), 200, "Proxy should return 200");
    
    let body = response.text().await?;
    assert!(
        body.contains("Server address") || body.contains("nginx"),
        "Should get workshop container response"
    );
    
    info!("  ✅ Proxy communication working");
    Ok(())
}

async fn test_idle_tracking(client: &TestClient) -> Result<()> {
    info!("TEST: Idle time should increase without activity");
    
    let user_id = format!("idle-user-{}", uuid::Uuid::new_v4());
    let token = client.generate_test_token(&user_id)?;
    
    // Create pod
    client.hub_request(
        reqwest::Method::GET,
        &format!("/{}/test", client.hub_namespace()),
        Some(&token),
    ).await?;

    client.wait_for_pod_running(&user_id, Duration::from_secs(60)).await?;
    tokio::time::sleep(Duration::from_secs(3)).await;
    
    // Check initial idle
    let health1 = client.check_workshop_health(&user_id).await?;
    let idle1 = health1["idle_seconds"].as_u64().unwrap();
    
    // Wait
    info!("  Waiting 4 seconds...");
    tokio::time::sleep(Duration::from_secs(4)).await;
    
    // Check idle again
    let health2 = client.check_workshop_health(&user_id).await?;
    let idle2 = health2["idle_seconds"].as_u64().unwrap();
    
    assert!(idle2 > idle1 + 3, "Idle time should increase (was {}, now {})", idle1, idle2);
    
    info!("  ✅ Idle tracking working");
    info!("     Initial: {}s, After 4s: {}s", idle1, idle2);
    
    Ok(())
}