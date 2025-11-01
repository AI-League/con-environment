// crates/hub/tests/helpers.rs
// Common test utilities

use k8s_openapi::api::core::v1::{Pod, Service};
use kube::{api::DeleteParams, Api, Client};
use serde_json::json;
use uuid::Uuid;
use std::sync::Arc;
use crate::{auth, config, AppState};

use crate::tests::config::{get_test_config, get_gc_test_config, get_stress_test_config, validate_talos_environment, TalosTestEnv};

pub struct TestContext {
    pub client: Client,
    pub config: Arc<config::Config>,
    pub state: AppState,
}

impl TestContext {
    pub async fn new() -> Self {
        // Validate we're in the Talos environment
        validate_talos_environment()
            .expect("Not running in Talos environment. See README for setup instructions.");
        
        let client = Client::try_default()
            .await
            .expect("Failed to create test Kubernetes client. Is the Talos cluster running?");
        
        let config = get_test_config();
        
        let auth_keys = Arc::new(auth::AuthKeys::new(b"test-secret-key"));
        
        let http_client = hyper_util::client::legacy::Client::builder(
            hyper_util::rt::TokioExecutor::new()
        ).build_http();
        
        let state = AppState {
            kube_client: client.clone(),
            auth_keys,
            http_client,
            config: config.clone(),
        };
        
        Self {
            client,
            config,
            state,
        }
    }
    
    /// Create a test context with GC-specific configuration (shorter timeouts)
    pub async fn new_for_gc() -> Self {
        validate_talos_environment()
            .expect("Not running in Talos environment");
        
        let client = Client::try_default()
            .await
            .expect("Failed to create test Kubernetes client");
        
        let config = get_gc_test_config();
        
        let auth_keys = Arc::new(auth::AuthKeys::new(b"test-secret-key"));
        
        let http_client = hyper_util::client::legacy::Client::builder(
            hyper_util::rt::TokioExecutor::new()
        ).build_http();
        
        let state = AppState {
            kube_client: client.clone(),
            auth_keys,
            http_client,
            config: config.clone(),
        };
        
        Self {
            client,
            config,
            state,
        }
    }
    
    pub fn generate_token(&self, username: &str) -> String {
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
        
        encode(&Header::default(), &claims, &self.state.auth_keys.encoding)
            .expect("Failed to encode test token")
    }
    
    pub async fn cleanup(&self) {
        use kube::api::ListParams;
        
        let pod_api: Api<Pod> = Api::namespaced(
            self.client.clone(),
            &self.config.workshop_namespace
        );
        
        let list_params = ListParams::default().labels(&format!(
            "workshop-hub/workshop-name={},app.kubernetes.io/managed-by=workshop-hub",
            self.config.workshop_name
        ));
        
        if let Ok(pods) = pod_api.list(&list_params).await {
            for pod in pods.items {
                if let Some(name) = pod.metadata.name {
                    let _ = pod_api.delete(&name, &DeleteParams::default()).await;
                }
            }
        }
        
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    }
    
    pub async fn create_test_pod(&self, user_id: &str) -> Result<Pod, kube::Error> {
        let pod_api: Api<Pod> = Api::namespaced(
            self.client.clone(),
            &self.config.workshop_namespace
        );
        
        let pod_name = format!("test-pod-{}", user_id);
        
        let pod: Pod = serde_json::from_value(json!({
            "apiVersion": "v1",
            "kind": "Pod",
            "metadata": {
                "name": pod_name,
                "labels": {
                    "workshop-hub/user-id": user_id,
                    "workshop-hub/workshop-name": self.config.workshop_name,
                    "app.kubernetes.io/managed-by": "workshop-hub"
                }
            },
            "spec": {
                "containers": [{
                    "name": "test",
                    "image": "nginx:alpine",
                    "ports": [{"containerPort": 80}]
                }]
            }
        })).unwrap();
        
        pod_api.create(&kube::api::PostParams::default(), &pod).await
    }
    
    pub async fn wait_for_pod_running(&self, pod_name: &str) -> Result<Option<Pod>, kube::Error> {
        use kube::runtime::wait::{await_condition, conditions};
        
        let pod_api: Api<Pod> = Api::namespaced(
            self.client.clone(),
            &self.config.workshop_namespace
        );
        
        let timeout = tokio::time::Duration::from_secs(120);
        let running = await_condition(pod_api, pod_name, conditions::is_pod_running());
        
        match tokio::time::timeout(timeout, running).await {
            Ok(Ok(pod)) => Ok(pod),
            Ok(Err(e)) => Err(kube::Error::Api(kube::error::ErrorResponse {
                status: "Error".to_string(),
                message: format!("Failed to wait for pod {}: {}", pod_name, e),
                reason: "WaitError".to_string(),
                code: 500,
            })),
            Err(_) => Err(kube::Error::Api(kube::error::ErrorResponse {
                status: "Timeout".to_string(),
                message: format!("Pod {} did not become running in time", pod_name),
                reason: "Timeout".to_string(),
                code: 408,
            }))
        }
    }
    
    pub async fn pod_exists(&self, pod_name: &str) -> bool {
        let pod_api: Api<Pod> = Api::namespaced(
            self.client.clone(),
            &self.config.workshop_namespace
        );
        
        pod_api.get(pod_name).await.is_ok()
    }
    
    pub async fn service_exists(&self, service_name: &str) -> bool {
        let svc_api: Api<Service> = Api::namespaced(
            self.client.clone(),
            &self.config.workshop_namespace
        );
        
        svc_api.get(service_name).await.is_ok()
    }
    
    pub async fn count_managed_pods(&self) -> usize {
        use kube::api::ListParams;
        
        let pod_api: Api<Pod> = Api::namespaced(
            self.client.clone(),
            &self.config.workshop_namespace
        );
        
        let list_params = ListParams::default().labels(&format!(
            "workshop-hub/workshop-name={},app.kubernetes.io/managed-by=workshop-hub",
            self.config.workshop_name
        ));
        
        pod_api
            .list(&list_params)
            .await
            .map(|list| list.items.len())
            .unwrap_or(0)
    }
}

/// Mock HTTP client for testing without actual network calls
pub mod mock_http {
    use axum::response::Response;
    use hyper::StatusCode;

    
    pub fn mock_health_response(idle_seconds: u64) -> Response<axum::body::Body> {
        let body = serde_json::json!({
            "status": "ok",
            "last_activity_timestamp": 1234567890,
            "idle_seconds": idle_seconds
        }).to_string();
        
        Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/json")
            .body(body.into())
            .unwrap()
    }
}