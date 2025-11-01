use anyhow::{Context, Result};
use k8s_openapi::api::core::v1::{Namespace, Pod, Service};
use kube::{Api, Client, ResourceExt};
use kube::api::{DeleteParams, ListParams};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, info};

/// Configuration loaded from environment or defaults
#[derive(Debug, Clone)]
pub struct TestConfig {
    pub hub_namespace: String,
    pub workshop_namespace: String,
    pub hub_service_name: String,
    pub hub_port: u16,
    pub workshop_name: String,
}

impl TestConfig {
    pub fn from_env() -> Self {
        Self {
            hub_namespace: std::env::var("HUB_NAMESPACE")
                .unwrap_or_else(|_| "workshop-hub-system".to_string()),
            workshop_namespace: std::env::var("WORKSHOP_NAMESPACE")
                .unwrap_or_else(|_| "test-workshops".to_string()),
            hub_service_name: std::env::var("HUB_SERVICE_NAME")
                .unwrap_or_else(|_| "workshop-hub".to_string()),
            hub_port: std::env::var("HUB_PORT")
                .unwrap_or_else(|_| "8080".to_string())
                .parse()
                .unwrap_or(8080),
            workshop_name: std::env::var("WORKSHOP_NAME")
                .unwrap_or_else(|_| "test-workshop".to_string()),
        }
    }
}

/// Main test client that interacts with the deployed system
pub struct TestClient {
    kube_client: Client,
    http_client: reqwest::Client,
    config: TestConfig,
    test_id: String,
}

impl TestClient {
    pub async fn new() -> Result<Self> {
        let kube_client = Client::try_default()
            .await
            .context("Failed to create Kubernetes client")?;

        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .context("Failed to create HTTP client")?;

        let config = TestConfig::from_env();
        let test_id = uuid::Uuid::new_v4().to_string()[..8].to_string();

        Ok(Self {
            kube_client,
            http_client,
            config,
            test_id,
        })
    }

    pub fn hub_namespace(&self) -> &str {
        &self.config.hub_namespace
    }

    pub fn workshop_namespace(&self) -> &str {
        &self.config.workshop_namespace
    }

    pub fn cluster_info(&self) -> String {
        // Get cluster info from kube config
        "connected".to_string() // Simplified for now
    }

    /// Verify the deployment is healthy before running tests
    pub async fn verify_deployment(&self) -> Result<()> {
        info!("Verifying hub namespace exists...");
        let ns_api: Api<Namespace> = Api::all(self.kube_client.clone());
        ns_api.get(&self.config.hub_namespace).await
            .context("Hub namespace not found")?;

        info!("Verifying workshop namespace exists...");
        ns_api.get(&self.config.workshop_namespace).await
            .context("Workshop namespace not found")?;

        info!("Verifying hub service exists...");
        let svc_api: Api<Service> = Api::namespaced(
            self.kube_client.clone(),
            &self.config.hub_namespace
        );
        svc_api.get(&self.config.hub_service_name).await
            .context("Hub service not found")?;

        info!("Verifying hub pods are running...");
        let pod_api: Api<Pod> = Api::namespaced(
            self.kube_client.clone(),
            &self.config.hub_namespace
        );
        
        let list_params = ListParams::default().labels("app=workshop-hub");
        let pods = pod_api.list(&list_params).await?;
        
        if pods.items.is_empty() {
            anyhow::bail!("No hub pods found");
        }

        for pod in pods.items {
            let name = pod.name_any();
            let phase = pod.status.as_ref()
                .and_then(|s| s.phase.as_deref())
                .unwrap_or("Unknown");
            
            info!("  Hub pod: {} - {}", name, phase);
            
            if phase != "Running" {
                anyhow::bail!("Hub pod {} is not running ({})", name, phase);
            }
        }

        info!("✅ Deployment verification passed");
        Ok(())
    }

    /// Generate a JWT token for testing
    pub fn generate_test_token(&self, username: &str) -> Result<String> {
        use jsonwebtoken::{encode, Header, EncodingKey};
        
        let claims = TestClaims {
            sub: username.to_string(),
            id: uuid::Uuid::new_v4(),
            exp: (chrono::Utc::now().timestamp() + 3600) as usize,
            iat: chrono::Utc::now().timestamp() as usize,
        };

        // Use test secret - should match what's deployed
        let secret = std::env::var("JWT_SECRET")
            .unwrap_or_else(|_| "test-secret-key".to_string());
        
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(secret.as_bytes())
        )?;

        Ok(token)
    }

    /// Get the hub service URL
    pub fn hub_url(&self) -> String {
        format!(
            "http://{}.{}.svc.cluster.local:{}",
            self.config.hub_service_name,
            self.config.hub_namespace,
            self.config.hub_port
        )
    }

    /// Make authenticated request to hub
    pub async fn hub_request(
        &self,
        method: reqwest::Method,
        path: &str,
        token: Option<&str>,
    ) -> Result<reqwest::Response> {
        let url = format!("{}{}", self.hub_url(), path);
        debug!("Request: {} {}", method, url);

        let mut req = self.http_client.request(method, &url);
        
        if let Some(token) = token {
            req = req.header("Authorization", format!("Bearer {}", token));
        }

        let response = req.send().await
            .context("Failed to send request to hub")?;

        Ok(response)
    }

    /// Get workshop pod for a user
    pub async fn get_workshop_pod(&self, user_id: &str) -> Result<Option<Pod>> {
        let pod_api: Api<Pod> = Api::namespaced(
            self.kube_client.clone(),
            &self.config.workshop_namespace
        );

        let list_params = ListParams::default().labels(&format!(
            "workshop-hub/user-id={},workshop-hub/workshop-name={}",
            user_id, self.config.workshop_name
        ));

        let pods = pod_api.list(&list_params).await?;
        Ok(pods.items.into_iter().next())
    }

    /// Get workshop service for a user
    pub async fn get_workshop_service(&self, user_id: &str) -> Result<Option<Service>> {
        let svc_api: Api<Service> = Api::namespaced(
            self.kube_client.clone(),
            &self.config.workshop_namespace
        );

        let list_params = ListParams::default().labels(&format!(
            "workshop-hub/user-id={},workshop-hub/workshop-name={}",
            user_id, self.config.workshop_name
        ));

        let services = svc_api.list(&list_params).await?;
        Ok(services.items.into_iter().next())
    }

    /// Check health of a workshop pod's sidecar
    pub async fn check_workshop_health(&self, user_id: &str) -> Result<serde_json::Value> {
        let service = self.get_workshop_service(user_id).await?
            .context("Service not found")?;
        
        let service_name = service.name_any();
        let url = format!(
            "http://{}.{}.svc.cluster.local:8080/health",
            service_name, self.config.workshop_namespace
        );

        let response = self.http_client.get(&url).send().await?;
        let json = response.json().await?;
        Ok(json)
    }

    /// Send request through workshop proxy
    pub async fn proxy_to_workshop(&self, user_id: &str, path: &str) -> Result<reqwest::Response> {
        let service = self.get_workshop_service(user_id).await?
            .context("Service not found")?;
        
        let service_name = service.name_any();
        let url = format!(
            "http://{}.{}.svc.cluster.local:8888{}",
            service_name, self.config.workshop_namespace, path
        );

        let response = self.http_client.get(&url).send().await?;
        Ok(response)
    }

    /// Wait for a pod to be running
    pub async fn wait_for_pod_running(&self, user_id: &str, timeout: Duration) -> Result<Pod> {
        let start = std::time::Instant::now();
        
        loop {
            if start.elapsed() > timeout {
                anyhow::bail!("Timeout waiting for pod to be running");
            }

            if let Some(pod) = self.get_workshop_pod(user_id).await? {
                let phase = pod.status.as_ref()
                    .and_then(|s| s.phase.as_deref())
                    .unwrap_or("Unknown");
                
                if phase == "Running" {
                    return Ok(pod);
                }

                debug!("Pod phase: {}", phase);
            }

            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }

    /// Count workshop pods
    pub async fn count_workshop_pods(&self) -> Result<usize> {
        let pod_api: Api<Pod> = Api::namespaced(
            self.kube_client.clone(),
            &self.config.workshop_namespace
        );

        let list_params = ListParams::default().labels(&format!(
            "workshop-hub/workshop-name={},app.kubernetes.io/managed-by=workshop-hub",
            self.config.workshop_name
        ));

        let pods = pod_api.list(&list_params).await?;
        Ok(pods.items.len())
    }

    /// Cleanup test resources
    pub async fn cleanup_test_resources(&self) -> Result<()> {
        info!("Cleaning up test resources...");

        let pod_api: Api<Pod> = Api::namespaced(
            self.kube_client.clone(),
            &self.config.workshop_namespace
        );

        // Delete pods with test label
        let list_params = ListParams::default().labels(&format!(
            "workshop-hub/test-id={}",
            self.test_id
        ));

        let pods = pod_api.list(&list_params).await?;
        for pod in pods.items {
            let name = pod.name_any();
            info!("  Deleting test pod: {}", name);
            let _ = pod_api.delete(&name, &DeleteParams::default()).await;
        }

        Ok(())
    }

    /// Tag a resource with test ID for cleanup
    pub fn test_label(&self) -> HashMap<String, String> {
        let mut labels = HashMap::new();
        labels.insert("workshop-hub/test-id".to_string(), self.test_id.clone());
        labels
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct TestClaims {
    sub: String,
    id: uuid::Uuid,
    exp: usize,
    iat: usize,
}