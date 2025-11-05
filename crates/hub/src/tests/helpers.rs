use k8s_openapi::api::core::v1::{Pod, Service, Namespace};
use kube::{api::{DeleteParams, PostParams, ListParams}, Api, Client};
use serde_json::json;
use std::sync::Arc;
use std::collections::BTreeMap;
use std::time::Duration;
use crate::{auth, config, AppState};
use super::config::{get_test_config, get_gc_test_config, validate_talos_environment};

/// Main test context that encapsulates all test dependencies
pub struct TestContext {
    pub client: Client,
    pub config: Arc<config::Config>,
    pub state: AppState,
    pub test_namespace: String,
}

impl TestContext {
    /// Create a standard test context
    /// test_name should be the name of the calling test function
    pub async fn new(test_name: &str) -> Self {
        Self::with_config(get_test_config(), test_name).await
    }
    
    /// Create a test context optimized for GC testing
    pub async fn new_for_gc(test_name: &str) -> Self {
        Self::with_config(get_gc_test_config(), test_name).await
    }
    
    /// Create a test context with a specific configuration
    async fn with_config(config: Arc<config::Config>, test_name: &str) -> Self {
        validate_talos_environment()
            .expect("Not running in Talos environment. See README for setup instructions.");
        
        let client = Client::try_default()
            .await
            .expect("Failed to create test Kubernetes client. Is the Talos cluster running?");
        
        // Create a consistent namespace name for this test (no random suffix)
        // This means the same test always uses the same namespace
        let test_namespace = format!("test-{}", 
            test_name.to_lowercase().replace('_', "-")
        );
        
        // Try to create the namespace (idempotent)
        let ns_api: Api<Namespace> = Api::all(client.clone());
        let namespace: Namespace = serde_json::from_value(json!({
            "apiVersion": "v1",
            "kind": "Namespace",
            "metadata": {
                "name": test_namespace,
                "labels": {
                    "workshop-hub/test": "true",
                    "workshop-hub/test-name": test_name
                }
            }
        })).unwrap();
        
        // Create if it doesn't exist, ignore if it already exists
        match ns_api.create(&PostParams::default(), &namespace).await {
            Ok(_) => {
                tracing::info!("Created test namespace: {}", test_namespace);
                // Wait for namespace to be ready
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
            Err(kube::Error::Api(err)) if err.code == 409 => {
                // Namespace already exists, that's fine
                tracing::info!("Using existing test namespace: {}", test_namespace);
            }
            Err(e) => {
                panic!("Failed to create test namespace: {}", e);
            }
        }
        
        // Update config to use this namespace
        let mut config_clone = (*config).clone();
        config_clone.workshop_namespace = test_namespace.clone();
        // Make workshop_name unique to this namespace
        config_clone.workshop_name = format!("{}-test", config_clone.workshop_name);
        let config = Arc::new(config_clone);
    
        
        let http_client = hyper_util::client::legacy::Client::builder(
            hyper_util::rt::TokioExecutor::new()
        ).build_http();
        
        let state = AppState {
            kube_client: client.clone(),
            http_client,
            config: config.clone(),
        };
        
        let ctx = Self {
            client,
            config,
            state,
            test_namespace,
        };
        
        // Clear the namespace before starting the test
        ctx.clear().await;
        
        ctx
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
            exp: (now + 3600) as usize,
            iat: now as usize,
        };
        
        encode(&Header::default(), &claims, &self.state.auth_keys.encoding)
            .expect("Failed to encode test token")
    }
    
    /// Clear all resources in the test namespace (but keep the namespace)
    /// This is called automatically when creating a test context
    pub async fn clear(&self) {
        tracing::info!("Clearing test namespace: {}", self.test_namespace);
        
        // Delete all pods
        let pod_api: Api<Pod> = Api::namespaced(
            self.client.clone(),
            &self.test_namespace
        );
        
        if let Ok(pods) = pod_api.list(&ListParams::default()).await {
            for pod in pods.items {
                if let Some(name) = pod.metadata.name {
                    let _ = pod_api.delete(&name, &DeleteParams::default()).await;
                }
            }
        }
        
        // Delete all services
        let svc_api: Api<Service> = Api::namespaced(
            self.client.clone(),
            &self.test_namespace
        );
        
        if let Ok(services) = svc_api.list(&ListParams::default()).await {
            for service in services.items {
                if let Some(name) = service.metadata.name {
                    let _ = svc_api.delete(&name, &DeleteParams::default()).await;
                }
            }
        }
        
        // Wait for deletions to complete
        tokio::time::sleep(Duration::from_secs(2)).await;
        
        tracing::info!("Test namespace cleared: {}", self.test_namespace);
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
        // Add app label for service selector (matches orchestrator pattern)
        labels.insert("app".to_string(), pod_name.clone());
        
        let pod: Pod = serde_json::from_value(json!({
            "apiVersion": "v1",
            "kind": "Pod",
            "metadata": {
                "name": pod_name,
                "labels": labels
            },
            "spec": {
                "restartPolicy": "Never",
                "containers": [
                    {
                        "name": "user-app",
                        "image": self.config.workshop_image.clone(),
                        "ports": [{"containerPort": self.config.workshop_port}]
                    },
                    {
                        "name": "sidecar",
                        "image": crate::SIDECAR,
                        "env": [
                            {
                                "name": "TARGET_PORT",
                                "value": self.config.workshop_port.to_string()
                            }
                        ],
                        "ports": [
                            {"containerPort": 8888, "name": "proxy"},
                            {"containerPort": 8080, "name": "health"}
                        ]
                    }
                ]
            }
        })).unwrap();
        
        pod_api.create(&PostParams::default(), &pod).await
    }
    
    /// Create a test service for a user pod
    pub async fn create_test_service(&self, user_id: &str) -> Result<Service, kube::Error> {
        let pod_api: Api<Pod> = Api::namespaced(
            self.client.clone(),
            &self.config.workshop_namespace
        );
        
        let svc_api: Api<Service> = Api::namespaced(
            self.client.clone(),
            &self.config.workshop_namespace
        );
        
        let service_name = format!("{}-{}", self.config.workshop_name, user_id);
        let pod_name = service_name.clone();
        
        // Get the pod to create an owner reference
        let pod = pod_api.get(&pod_name).await?;
        let pod_uid = pod.metadata.uid.clone()
            .ok_or_else(|| kube::Error::Api(kube::error::ErrorResponse {
                status: "Error".to_string(),
                message: "Pod UID not found".to_string(),
                reason: "PodUIDMissing".to_string(),
                code: 500,
            }))?;
        
        let mut labels = BTreeMap::new();
        labels.insert("workshop-hub/user-id".to_string(), user_id.to_string());
        labels.insert("workshop-hub/workshop-name".to_string(), self.config.workshop_name.clone());
        labels.insert("app.kubernetes.io/managed-by".to_string(), "workshop-hub".to_string());
        
        // Create owner reference so service is deleted when pod is deleted
        let owner_ref = serde_json::json!({
            "apiVersion": "v1",
            "kind": "Pod",
            "name": pod_name,
            "uid": pod_uid,
            "controller": false,
            "blockOwnerDeletion": false
        });
        
        let service: Service = serde_json::from_value(json!({
            "apiVersion": "v1",
            "kind": "Service",
            "metadata": {
                "name": service_name,
                "labels": labels,
                "ownerReferences": [owner_ref]
            },
            "spec": {
                "selector": {
                    "app": pod_name
                },
                "ports": [
                    {
                        "name": "proxy",
                        "protocol": "TCP",
                        "port": 8888,
                        "targetPort": 8888
                    },
                    {
                        "name": "health",
                        "protocol": "TCP",
                        "port": 8080,
                        "targetPort": 8080
                    }
                ]
            }
        })).unwrap();
        
        svc_api.create(&PostParams::default(), &service).await
    }
    
    /// Wait for a pod to reach running state
    pub async fn wait_for_pod_running(&self, pod_name: &str) -> Result<(), kube::Error> {
        let pod_api: Api<Pod> = Api::namespaced(
            self.client.clone(),
            &self.config.workshop_namespace
        );
        
        for _ in 0..60 {
            if !self.pod_exists(pod_name).await {
                return Err(kube::Error::Api(kube::error::ErrorResponse {
                    status: format!("Pod {} was deleted", pod_name),
                    message: format!("Pod {} was deleted while waiting", pod_name),
                    reason: "Deleted".to_string(),
                    code: 410,
                }));
            }
            
            if let Ok(pod) = pod_api.get(pod_name).await {
                if let Some(status) = &pod.status {
                    if let Some(phase) = &status.phase {
                        if phase == "Running" {
                            return Ok(());
                        }
                        if phase == "Failed" || phase == "Unknown" {
                            return Err(kube::Error::Api(kube::error::ErrorResponse {
                                status: format!("Pod {} entered {} state", pod_name, phase),
                                message: format!("Pod {} did not reach running state", pod_name),
                                reason: phase.clone(),
                                code: 500,
                            }));
                        }
                    }
                }
            }
            
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        
        Err(kube::Error::Api(kube::error::ErrorResponse {
            status: "Timeout".to_string(),
            message: format!("Pod {} did not become running in time", pod_name),
            reason: "Timeout".to_string(),
            code: 408,
        }))
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

    /// Make an HTTP request to a service in the test namespace
    /// This is useful for testing pod communication
    pub async fn http_get_service(
        &self,
        user_id: &str,
        port: u16,
        path: &str,
    ) -> Result<reqwest::Response, reqwest::Error> {
        let service_name = format!("{}-{}", self.config.workshop_name, user_id);
        let url = format!(
            "http://{}.{}.svc.cluster.local:{}{}",
            service_name,
            self.config.workshop_namespace,
            port,
            path
        );
        
        tracing::debug!("HTTP GET: {}", url);
        
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()?;
        
        client.get(&url).send().await
    }
    
    /// Check the health endpoint of a pod's sidecar
    pub async fn check_pod_health(&self, user_id: &str) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let response = self.http_get_service(user_id, 8080, "/health").await?;
        
        if !response.status().is_success() {
            return Err(format!("Health check failed with status: {}", response.status()).into());
        }
        
        let json = response.json().await?;
        Ok(json)
    }
    
    /// Send a request through the sidecar proxy to the workshop container
    pub async fn proxy_to_workshop(&self, user_id: &str, path: &str) -> Result<reqwest::Response, reqwest::Error> {
        self.http_get_service(user_id, 8888, path).await
    }
}

// Namespaces persist in test environment - no automatic cleanup needed

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
