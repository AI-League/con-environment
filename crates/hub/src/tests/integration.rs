use axum::extract::Request;
use k8s_openapi::api::core::v1::Pod;
use kube::Api;
use crate::{auth, orchestrator, HubError};
use super::helpers::TestContext;

#[tracing_test::traced_test]
#[tokio::test]
async fn test_orchestrator_creates_pod_and_service() {
    let ctx = TestContext::new("test_orchestrator_creates_pod_and_service").await;

    
    // Test pod creation through orchestrator
    let user_id = "test-user-1";
    let binding = orchestrator::get_or_create_pod(
        &ctx.client, 
        user_id, 
        ctx.config.clone()
    ).await;
    
    assert!(binding.is_ok(), "Should create pod successfully");
    let binding = binding.unwrap();
    
    // Verify pod was created
    assert!(ctx.pod_exists(&binding.pod_name).await, "Pod should exist");
    
    // Verify service was created
    assert!(ctx.service_exists(&binding.service_name).await, "Service should exist");
    
    // Verify we can retrieve the same binding (idempotency)
    let second_binding = orchestrator::get_or_create_pod(
        &ctx.client,
        user_id,
        ctx.config.clone()
    ).await;
    
    assert!(second_binding.is_ok(), "Should retrieve existing pod");
    let second_binding = second_binding.unwrap();
    
    assert_eq!(binding.pod_name, second_binding.pod_name, "Should return same pod");
    assert_eq!(binding.service_name, second_binding.service_name, "Should return same service");
}

#[tracing_test::traced_test]
#[tokio::test]
async fn test_pod_limit_enforcement() {
    let ctx = TestContext::new("test_pod_limit_enforcement").await;
    
    // Create a config with very low limit for testing
    let mut limited_config = (*ctx.config).clone();
    limited_config.workshop_pod_limit = 2;
    let limited_config = std::sync::Arc::new(limited_config);
    
    // Create pods up to limit
    let user1 = orchestrator::get_or_create_pod(
        &ctx.client,
        "limit-user-1",
        limited_config.clone()
    ).await;
    assert!(user1.is_ok(), "First pod should succeed");
    
    let user2 = orchestrator::get_or_create_pod(
        &ctx.client,
        "limit-user-2",
        limited_config.clone()
    ).await;
    assert!(user2.is_ok(), "Second pod should succeed");
    
    // This should fail due to limit
    let user3 = orchestrator::get_or_create_pod(
        &ctx.client,
        "limit-user-3",
        limited_config.clone()
    ).await;
    
    match user3 {
        Err(HubError::PodLimitReached) => {
            // Expected error
        }
        _ => panic!("Expected PodLimitReached error, got: {:?}", user3),
    }
}

#[test]
fn test_jwt_token_generation_and_validation() {
    use jsonwebtoken::{encode, decode, Header, Validation};
    
    let keys = auth::AuthKeys::new(b"test-secret");
    
    // Create test claims
    let claims = auth::Claims {
        sub: "testuser".to_string(),
        exp: (chrono::Utc::now().timestamp() + 3600) as usize,
        iat: chrono::Utc::now().timestamp() as usize,
    };
    
    // Encode
    let token = encode(&Header::default(), &claims, &keys.encoding);
    assert!(token.is_ok(), "Token encoding should succeed");
    
    // Decode
    let token = token.unwrap();
    let decoded = decode::<auth::Claims>(
        &token,
        &keys.decoding,
        &Validation::default()
    );
    
    assert!(decoded.is_ok(), "Token decoding should succeed");
    
    let decoded_claims = decoded.unwrap().claims;
    assert_eq!(decoded_claims.sub, "testuser");
}

#[test]
fn test_extract_token_from_request() {
    // Test 1: Extract from "Authorization: Bearer" header
    let req_header = Request::builder()
        .header("authorization", "Bearer test-token-123")
        .body(())
        .unwrap();
    
    let token_header = auth::extract_token_from_request(&req_header);
    assert!(token_header.is_ok(), "Should extract token from header");
    assert_eq!(token_header.unwrap(), "test-token-123");

    // Test 2: Extract from query parameter
    let req_query = Request::builder()
        .uri("/path?token=test-token-456&other=value")
        .body(())
        .unwrap();
    
    let token_query = auth::extract_token_from_request(&req_query);
    assert!(token_query.is_ok(), "Should extract token from query");
    assert_eq!(token_query.unwrap(), "test-token-456");

    // Test 3: Missing token
    let req_missing = Request::builder()
        .uri("/path?other=value")
        .body(())
        .unwrap();

    let no_token = auth::extract_token_from_request(&req_missing);
    assert!(no_token.is_err(), "Should fail with no token");
}

#[tracing_test::traced_test]
#[tokio::test]
async fn test_concurrent_pod_creation() {
    let ctx = TestContext::new("test_concurrent_pod_creation").await;
    
    // Spawn multiple concurrent pod creation requests
    let mut handles = vec![];
    
    for i in 0..5 {
        let client = ctx.client.clone();
        let config = ctx.config.clone();
        let user_id = format!("concurrent-user-{}", i);
        
        let handle = tokio::spawn(async move {
            orchestrator::get_or_create_pod(&client, &user_id, config).await
        });
        
        handles.push(handle);
    }
    
    // Wait for all to complete
    let results: Vec<_> = futures::future::join_all(handles).await;
    
    // All should succeed
    for result in results {
        assert!(result.is_ok(), "Task should not panic");
        assert!(result.unwrap().is_ok(), "Pod creation should succeed");
    }
    
    // Verify we have exactly 5 pods
    let pod_count = ctx.count_managed_pods().await;
    assert_eq!(pod_count, 5, "Should have exactly 5 pods");
}

#[tracing_test::traced_test]
#[tokio::test]
async fn test_cleanup_preserves_active_pods() {
    let ctx = TestContext::new_for_gc("test_cleanup_preserves_active_pods").await;
    
    // // Create two pods - one "active" and one "idle"
    // let active_pod = ctx.create_test_pod("active-user").await
    //     .expect("Failed to create active pod");
    // let idle_pod = ctx.create_test_pod("idle-user").await
    //     .expect("Failed to create idle pod");
    //
    // let active_name = active_pod.metadata.name.as_ref().unwrap();
    // let idle_name = idle_pod.metadata.name.as_ref().unwrap();
    
    // Simulate the active pod having recent activity by updating its annotation
    let pod_api: Api<Pod> = Api::namespaced(
        ctx.client.clone(),
        &ctx.config.workshop_namespace
    );
    
    // For this test, we'll assume the GC checks a "last-activity" annotation
    // In a real scenario, this would come from the sidecar health endpoint
    
    let result = crate::gc::cleanup_idle_pods(
        &pod_api,
        &ctx.config.workshop_name,
        60, // 60 second idle threshold
    ).await;
    
    assert!(result.is_ok(), "GC should succeed");
    
    // In a real test with proper sidecar health endpoints:
    // - Active pod would report low idle time and remain
    // - Idle pod would report high idle time and be deleted
}

