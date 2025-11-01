// crates/hub/tests/integration_tests.rs

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use k8s_openapi::api::core::v1::{Pod, Service};
use kube::{api::ListParams, Api, Client};
use serde_json::json;
use uuid::Uuid;
use std::sync::Arc;
use tower::ServiceExt;

// Re-export from the main crate
use crate::{auth, config, orchestrator, AppState, HubError};

/// Helper to create a test config
fn test_config() -> Arc<config::Config> {
    Arc::new(config::Config {
        workshop_name: "test-workshop".to_string(),
        workshop_namespace: "test-ns".to_string(),
        workshop_ttl_seconds: 3600,
        workshop_idle_seconds: 600,
        workshop_image: "nginx:alpine".to_string(),
        workshop_port: 80,
        workshop_pod_limit: 5,
        workshop_cpu_request: "100m".to_string(),
        workshop_cpu_limit: "500m".to_string(),
        workshop_mem_request: "128Mi".to_string(),
        workshop_mem_limit: "512Mi".to_string(),
    })
}

/// Helper to create test app state
async fn test_app_state() -> AppState {
    let kube_client = Client::try_default()
        .await
        .expect("Failed to create test Kubernetes client");
    
    let auth_keys = Arc::new(auth::AuthKeys::new(b"test-secret-key"));
    
    let http_client = hyper_util::client::legacy::Client::builder(
        hyper_util::rt::TokioExecutor::new()
    ).build_http();
    
    let config = test_config();
    
    AppState {
        kube_client,
        auth_keys,
        http_client,
        config,
    }
}

/// Helper to generate a valid JWT token
fn generate_test_token(state: &AppState, username: &str) -> String {
    use jsonwebtoken::{encode, Header};
    use std::time::{SystemTime, UNIX_EPOCH};
    
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    let claims = auth::Claims {
        sub: username.to_string(),
        id: Uuid::nil(),
        exp: (now + 3600) as usize,
        iat: now as usize,
    };
    
    encode(&Header::default(), &claims, &state.auth_keys.encoding)
        .expect("Failed to encode test token")
}

#[tokio::test]
#[ignore] // Remove this to run with actual k8s cluster
async fn test_auth_login() {
    let state = test_app_state().await;
    
    // Create a simple router just for login
    let app = axum::Router::new()
        .route("/login", axum::routing::post(auth::simple_login_handler))
        .with_state(state);
    
    // Create a login request
    let request = Request::builder()
        .method("POST")
        .uri("/login")
        .header("content-type", "application/json")
        .body(Body::from(json!({"username": "testuser"}).to_string()))
        .unwrap();
    
    // Send the request
    let response = app.oneshot(request).await.unwrap();
    
    // Check response
    assert_eq!(response.status(), StatusCode::OK);
    
    // Parse response body
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    
    // Verify token exists
    assert!(json.get("token").is_some());
    assert!(json["token"].as_str().unwrap().len() > 0);
}

#[tokio::test]
#[ignore] // Remove this to run with actual k8s cluster
async fn test_auth_middleware_valid_token() {
    let state = test_app_state().await;
    let token = generate_test_token(&state, "testuser");
    
    // Create a test route that requires auth
    let app = axum::Router::new()
        .route(
            "/protected",
            axum::routing::get(|claims: axum::Extension<auth::Claims>| async move {
                format!("Hello, {}!", claims.sub)
            }),
        )
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::auth_middleware,
        ))
        .with_state(state);
    
    // Request with valid token
    let request = Request::builder()
        .uri("/protected")
        .header("authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let text = String::from_utf8(body.to_vec()).unwrap();
    assert_eq!(text, "Hello, testuser!");
}

#[tokio::test]
#[ignore] // Remove this to run with actual k8s cluster
async fn test_auth_middleware_invalid_token() {
    let state = test_app_state().await;
    
    let app = axum::Router::new()
        .route("/protected", axum::routing::get(|| async { "Protected" }))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::auth_middleware,
        ))
        .with_state(state);
    
    // Request with invalid token
    let request = Request::builder()
        .uri("/protected")
        .header("authorization", "Bearer invalid-token")
        .body(Body::empty())
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[ignore] // Remove this to run with actual k8s cluster
async fn test_auth_middleware_no_token() {
    let state = test_app_state().await;
    
    let app = axum::Router::new()
        .route("/protected", axum::routing::get(|| async { "Protected" }))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::auth_middleware,
        ))
        .with_state(state);
    
    // Request without token
    let request = Request::builder()
        .uri("/protected")
        .body(Body::empty())
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[ignore] // Remove this to run with actual k8s cluster
async fn test_pod_creation() {
    let client = Client::try_default()
        .await
        .expect("Failed to create Kubernetes client");
    
    let config = test_config();
    let user_id = "test-user-123";
    
    // Clean up any existing test pods first
    cleanup_test_pods(&client, &config).await;
    
    // Create a pod
    let result = orchestrator::get_or_create_pod(&client, user_id, config.clone()).await;
    
    assert!(result.is_ok());
    let binding = result.unwrap();
    
    // Verify pod was created
    assert!(binding.pod_name.contains(user_id));
    assert!(binding.cluster_dns_name.contains(&config.workshop_namespace));
    
    // Verify pod exists in Kubernetes
    let pod_api: Api<Pod> = Api::namespaced(client.clone(), &config.workshop_namespace);
    let pod = pod_api.get(&binding.pod_name).await;
    assert!(pod.is_ok());
    
    // Verify service exists
    let svc_api: Api<Service> = Api::namespaced(client.clone(), &config.workshop_namespace);
    let service = svc_api.get(&binding.service_name).await;
    assert!(service.is_ok());
    
    // Clean up
    cleanup_test_pods(&client, &config).await;
}

#[tokio::test]
#[ignore] // Remove this to run with actual k8s cluster
async fn test_pod_reuse() {
    let client = Client::try_default()
        .await
        .expect("Failed to create Kubernetes client");
    
    let config = test_config();
    let user_id = "test-user-reuse";
    
    // Clean up first
    cleanup_test_pods(&client, &config).await;
    
    // Create a pod
    let first_result = orchestrator::get_or_create_pod(&client, user_id, config.clone()).await;
    assert!(first_result.is_ok());
    let first_binding = first_result.unwrap();
    
    // Try to "create" again - should reuse the existing pod
    let second_result = orchestrator::get_or_create_pod(&client, user_id, config.clone()).await;
    assert!(second_result.is_ok());
    let second_binding = second_result.unwrap();
    
    // Should be the same pod
    assert_eq!(first_binding.pod_name, second_binding.pod_name);
    assert_eq!(first_binding.service_name, second_binding.service_name);
    
    // Clean up
    cleanup_test_pods(&client, &config).await;
}

#[tokio::test]
#[ignore] // Remove this to run with actual k8s cluster
async fn test_pod_limit() {
    let client = Client::try_default()
        .await
        .expect("Failed to create Kubernetes client");
    
    // Create a config with a very low limit
    let mut config = test_config();
    Arc::get_mut(&mut config).unwrap().workshop_pod_limit = 2;
    
    cleanup_test_pods(&client, &config).await;
    
    // Create pods up to the limit
    let user1 = orchestrator::get_or_create_pod(&client, "limit-user-1", config.clone()).await;
    assert!(user1.is_ok());
    
    let user2 = orchestrator::get_or_create_pod(&client, "limit-user-2", config.clone()).await;
    assert!(user2.is_ok());
    
    // This one should fail
    let user3 = orchestrator::get_or_create_pod(&client, "limit-user-3", config.clone()).await;
    assert!(user3.is_err());
    
    if let Err(HubError::PodLimitReached) = user3 {
        // Expected error
    } else {
        panic!("Expected PodLimitReached error");
    }
    
    // Clean up
    cleanup_test_pods(&client, &config).await;
}

#[tokio::test]
#[ignore] // Remove this to run with actual k8s cluster
async fn test_config_validation() {
    let config = config::Config {
        workshop_name: "test".to_string(),
        workshop_namespace: "test-ns".to_string(),
        workshop_ttl_seconds: 3600,
        workshop_idle_seconds: 600,
        workshop_image: "nginx".to_string(),
        workshop_port: 80,
        workshop_pod_limit: 100,
        workshop_cpu_request: "100m".to_string(),
        workshop_cpu_limit: "500m".to_string(),
        workshop_mem_request: "128Mi".to_string(),
        workshop_mem_limit: "512Mi".to_string(),
    };
    
    // Verify defaults are sensible
    assert!(config.workshop_ttl_seconds > 0);
    assert!(config.workshop_idle_seconds > 0);
    assert!(config.workshop_pod_limit > 0);
    assert!(!config.workshop_image.is_empty());
}

/// Helper function to clean up test pods
async fn cleanup_test_pods(client: &Client, config: &config::Config) {
    use kube::api::DeleteParams;
    
    let pod_api: Api<Pod> = Api::namespaced(client.clone(), &config.workshop_namespace);
    let svc_api: Api<Service> = Api::namespaced(client.clone(), &config.workshop_namespace);
    
    let list_params = ListParams::default().labels(&format!(
        "workshop-hub/workshop-name={},app.kubernetes.io/managed-by=workshop-hub",
        config.workshop_name
    ));
    
    // Delete all test pods
    if let Ok(pods) = pod_api.list(&list_params).await {
        for pod in pods.items {
            if let Some(name) = pod.metadata.name {
                let _ = pod_api.delete(&name, &DeleteParams::default()).await;
            }
        }
    }
    
    // Wait a bit for cleanup
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
}

// Unit tests for individual components
    
#[test]
fn test_config_defaults() {
    use crate::config::Config;
    
    // Test that we can create a config with environment variables
    std::env::set_var("HUB_WORKSHOP_NAME", "test-workshop");
    std::env::set_var("HUB_WORKSHOP_NAMESPACE", "test-ns");
    
    let config = Config::from_env();
    assert!(config.is_ok());
    
    let config = config.unwrap();
    assert_eq!(config.workshop_name, "test-workshop");
    assert_eq!(config.workshop_namespace, "test-ns");
    
    // Clean up
    std::env::remove_var("HUB_WORKSHOP_NAME");
    std::env::remove_var("HUB_WORKSHOP_NAMESPACE");
}

#[test]
fn test_auth_keys() {
    let keys = auth::AuthKeys::new(b"test-secret");
    
    // Create a test claim
    use jsonwebtoken::{encode, decode, Header, Validation};
    
    let claims = auth::Claims {
        sub: "testuser".to_string(),
        id: Uuid::nil(),
        exp: (chrono::Utc::now().timestamp() + 3600) as usize,
        iat: chrono::Utc::now().timestamp() as usize,
    };
    
    // Encode
    let token = encode(&Header::default(), &claims, &keys.encoding);
    assert!(token.is_ok());
    
    // Decode
    let token = token.unwrap();
    let decoded = decode::<auth::Claims>(
        &token,
        &keys.decoding,
        &Validation::default()
    );
    assert!(decoded.is_ok());
    
    let decoded_claims = decoded.unwrap().claims;
    assert_eq!(decoded_claims.sub, "testuser");
}

#[test]
fn test_extract_token_from_headers() {
    use axum::http::HeaderMap;
    
    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        "Bearer test-token-123".parse().unwrap()
    );
    
    let token = auth::extract_token_from_headers(&headers);
    assert!(token.is_ok());
    assert_eq!(token.unwrap(), "test-token-123");
}

#[test]
fn test_extract_token_from_query() {
    let query = "token=test-token-456&other=value";
    let token = auth::extract_token_from_query(query);
    assert!(token.is_ok());
    assert_eq!(token.unwrap(), "test-token-456");
}
