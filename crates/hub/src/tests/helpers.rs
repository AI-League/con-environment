use k8s_openapi::api::core::v1::{Pod, Service, Container, PodSpec};
use kube::{api::{DeleteParams, PostParams, ListParams}, Api, Client};
use serde_json::json;
use uuid::Uuid;
use std::sync::Arc;
use std::collections::BTreeMap;
use std::time::Duration;
use crate::{auth, config, AppState};
use super::config::{get_test_config, get_gc_test_config, get_stress_test_config, validate_talos_environment};

/// Main test context that encapsulates all test dependencies
pub struct TestContext {
    pub client: Client,
    pub config: Arc<config::Config>,
    pub state: AppState,
}

impl TestContext {
    /// Create a standard test context
    pub async fn new() -> Self {
        Self::with_config(get_test_config()).await
    }
    
    /// Create a test context optimized for GC testing
    pub async fn new_for_gc() -> Self {
        Self::with_config(get_gc_test_config()).await
    }
    
    /// Create a test context for stress testing
    pub async fn new_for_stress() -> Self {
        Self::with_config(get_stress_test_config()).await
    }
    
    /// Create a test context with a specific configuration
    async fn with_config(config: Arc<config::Config>) -> Self {
        validate_talos_environment()
            .expect("Not running in Talos environment. See README for setup instructions.");
        
        let client = Client::try_default()
            .await
            .expect("Failed to create test Kubernetes client. Is the Talos cluster running?");
        
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
    
    /// Generate a test JWT token for a given username
    pub fn generate_token(&self, username: &str) -> String {
        use jsonwebtoken::{encode, Header};
        use std::time::{SystemTime, UNIX_EPOCH};
        
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let claims = auth::Claims {
            sub: username.to_string(),
            id: Uuid::new_v4(),
            exp: (now + 3600) as usize,
            iat: now as usize,
        };
        
        encode(&Header::default(), &claims, &self.state.auth_keys.encoding)
            .expect("Failed to encode test token")
    }
    
    /// Cleanup all test resources in the namespace
    pub async fn cleanup(&self) {
        self.cleanup_pods().await;
        self.cleanup_services().await;
    }
    
    /// Cleanup all test pods
    pub async fn cleanup_pods(&self) {
        let pod_api: Api<Pod> = Api::namespaced(
            self.client.clone(),
            &self.config.workshop_namespace
        );
        
        let list_params = ListParams::default()
            .labels(&format!(
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
        
        // Wait for deletions to propagate
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    
    /// Cleanup all test services
    pub async fn cleanup_services(&self) {
        let svc_api: Api<Service> = Api::namespaced(
            self.client.clone(),
            &self.config.workshop_namespace
        );
        
        let list_params = ListParams::default()
            .labels(&format!(
                "workshop-hub/workshop-name={},app.kubernetes.io/managed-by=workshop-hub",
                self.config.workshop_name
            ));
        
        if let Ok(services) = svc_api.list(&list_params).await {
            for service in services.items {
                if let Some(name) = service.metadata.name {
                    let _ = svc_api.delete(&name, &DeleteParams::default()).await;
                }
            }
        }
    }
    
    /// Create a test pod with standard labels
    pub async fn create_test_pod(&self, user_id: &str) -> Result<Pod, kube::Error> {
        let pod_api: Api<Pod> = Api::namespaced(
            self.client.clone(),
            &self.config.workshop_namespace
        );
        
        let pod_name = format!("{}-{}", self.config.workshop_name, user_id);
        
        let mut labels = BTreeMap::new();
        labels.insert("workshop-hub/user-id".to_string(), user_id.to_string());
        labels.insert("workshop-hub/workshop-name".to_string(), self.config.workshop_name.clone());
        labels.insert("app.kubernetes.io/managed-by".to_string(), "workshop-hub".to_string());
        
        let pod: Pod = serde_json::from_value(json!({
            "apiVersion": "v1",
            "kind": "Pod",
            "metadata": {
                "name": pod_name,
                "labels": labels
            },
            "spec": {
                "containers": [{
                    "name": "workshop",
                    "image": &self.config.workshop_image,
                    "ports": [{"containerPort": self.config.workshop_port}],
                    "resources": {
                        "requests": {
                            "cpu": &self.config.workshop_cpu_request,
                            "memory": &self.config.workshop_mem_request
                        },
                        "limits": {
                            "cpu": &self.config.workshop_cpu_limit,
                            "memory": &self.config.workshop_mem_limit
                        }
                    }
                }, {
                    "name": "sidecar",
                    "image": "workshop-sidecar:latest",
                    "imagePullPolicy": "Never",
                    "ports": [
                        {"containerPort": 8080, "name": "health"},
                        {"containerPort": 8888, "name": "proxy"}
                    ],
                    "env": [{
                        "name": "UPSTREAM_HOST",
                        "value": "127.0.0.1"
                    }, {
                        "name": "UPSTREAM_PORT",
                        "value": self.config.workshop_port.to_string()
                    }]
                }]
            }
        })).expect("Valid json");
        
        pod_api.create(&PostParams::default(), &pod).await
    }
    
    /// Create a test service
    pub async fn create_test_service(&self, user_id: &str) -> Result<Service, kube::Error> {
        let svc_api: Api<Service> = Api::namespaced(
            self.client.clone(),
            &self.config.workshop_namespace
        );
        
        let service_name = format!("{}-{}", self.config.workshop_name, user_id);
        
        let mut labels = BTreeMap::new();
        labels.insert("workshop-hub/user-id".to_string(), user_id.to_string());
        labels.insert("workshop-hub/workshop-name".to_string(), self.config.workshop_name.clone());
        labels.insert("app.kubernetes.io/managed-by".to_string(), "workshop-hub".to_string());
        
        let service: Service = serde_json::from_value(json!({
            "apiVersion": "v1",
            "kind": "Service",
            "metadata": {
                "name": service_name,
                "labels": labels.clone()
            },
            "spec": {
                "selector": labels,
                "ports": [
                    {
                        "name": "workshop",
                        "port": self.config.workshop_port,
                        "targetPort": self.config.workshop_port
                    },
                    {
                        "name": "health",
                        "port": 8080,
                        "targetPort": 8080
                    }
                ]
            }
        })).expect("Valid json");
        
        svc_api.create(&PostParams::default(), &service).await
    }
    
    /// Wait for a pod to reach Running state
    pub async fn wait_for_pod_running(&self, pod_name: &str) -> Result<(), kube::Error> {
        let pod_api: Api<Pod> = Api::namespaced(
            self.client.clone(),
            &self.config.workshop_namespace
        );
        
        let timeout = Duration::from_secs(60);
        let start = std::time::Instant::now();
        
        loop {
            if start.elapsed() > timeout {
                return Err(kube::Error::Api(kube::error::ErrorResponse {
                    status: "Timeout".to_string(),
                    message: format!("Pod {} did not become running in time", pod_name),
                    reason: "Timeout".to_string(),
                    code: 408,
                }));
            }
            
            match pod_api.get(pod_name).await {
                Ok(pod) => {
                    if let Some(status) = &pod.status {
                        if let Some(phase) = &status.phase {
                            if phase == "Running" {
                                return Ok(());
                            }
                        }
                    }
                }
                Err(e) => return Err(e),
            }
            
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
    
    /// Check if a pod exists
    pub async fn pod_exists(&self, pod_name: &str) -> bool {
        let pod_api: Api<Pod> = Api::namespaced(
            self.client.clone(),
            &self.config.workshop_namespace
        );
        
        pod_api.get(pod_name).await.is_ok()
    }
    
    /// Check if a service exists
    pub async fn service_exists(&self, service_name: &str) -> bool {
        let svc_api: Api<Service> = Api::namespaced(
            self.client.clone(),
            &self.config.workshop_namespace
        );
        
        svc_api.get(service_name).await.is_ok()
    }
    
    /// Count managed pods in the namespace
    pub async fn count_managed_pods(&self) -> usize {
        let pod_api: Api<Pod> = Api::namespaced(
            self.client.clone(),
            &self.config.workshop_namespace
        );
        
        let list_params = ListParams::default()
            .labels(&format!(
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

/// Mock HTTP responses for testing
pub mod mock {
    use axum::response::Response;
    use hyper::StatusCode;
    
    pub fn health_response(idle_seconds: u64) -> Response<axum::body::Body> {
        let body = serde_json::json!({
            "status": "ok",
            "last_activity_timestamp": chrono::Utc::now().timestamp() - idle_seconds as i64,
            "idle_seconds": idle_seconds
        }).to_string();
        
        Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/json")
            .body(body.into())
            .unwrap()
    }
    
    pub fn unhealthy_response() -> Response<axum::body::Body> {
        Response::builder()
            .status(StatusCode::SERVICE_UNAVAILABLE)
            .body("unhealthy".into())
            .unwrap()
    }
}
