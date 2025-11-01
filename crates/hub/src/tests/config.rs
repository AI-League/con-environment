// crates/hub/tests/config.rs
// Test-specific configuration optimized for the Talos cluster

use std::sync::Arc;
use crate::config::Config;

/// Get test configuration optimized for the Talos QEMU cluster
pub fn get_test_config() -> Arc<Config> {
    Arc::new(Config {
        workshop_name: std::env::var("TEST_WORKSHOP_NAME")
            .unwrap_or_else(|_| "test-workshop".to_string()),
        
        workshop_namespace: std::env::var("TEST_NAMESPACE")
            .unwrap_or_else(|_| "test-ns".to_string()),
        
        // Conservative limits for testing
        workshop_ttl_seconds: 3600, // 1 hour
        workshop_idle_seconds: 600,  // 10 minutes
        
        // Use nginx:alpine which should pull fast from your registry mirrors
        workshop_image: "nginx:alpine".to_string(),
        workshop_port: 80,
        
        // Conservative pod limit for testing
        workshop_pod_limit: std::env::var("TEST_POD_LIMIT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(5),
        
        // Resource requests/limits appropriate for the Talos cluster
        // Your workers have 12GB RAM and 4 CPUs, so these are safe
        workshop_cpu_request: "50m".to_string(),
        workshop_cpu_limit: "200m".to_string(),
        workshop_mem_request: "64Mi".to_string(),
        workshop_mem_limit: "256Mi".to_string(),
    })
}

/// Get configuration for stress testing (higher limits)
pub fn get_stress_test_config() -> Arc<Config> {
    let mut config = (*get_test_config()).clone();
    
    // Higher limits for stress testing
    config.workshop_pod_limit = std::env::var("STRESS_POD_LIMIT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(20);
    
    // Shorter timeouts for faster test cycles
    config.workshop_ttl_seconds = 300; // 5 minutes
    config.workshop_idle_seconds = 60; // 1 minute
    
    Arc::new(config)
}

/// Get configuration for GC testing (very short timeouts)
pub fn get_gc_test_config() -> Arc<Config> {
    let mut config = (*get_test_config()).clone();
    
    // Very short timeouts for GC testing
    config.workshop_ttl_seconds = 120; // 2 minutes
    config.workshop_idle_seconds = 30; // 30 seconds
    
    Arc::new(config)
}

/// Validate that we're running in the expected Talos environment
pub fn validate_talos_environment() -> Result<(), String> {
    // Check KUBECONFIG is set and points to the Talos cluster
    let kubeconfig = std::env::var("KUBECONFIG")
        .map_err(|_| "KUBECONFIG not set. Are you in the nix development shell?".to_string())?;
    
    if !kubeconfig.contains(".data/talos/kubeconfig") {
        return Err(format!(
            "KUBECONFIG doesn't point to Talos cluster: {}. Expected path containing '.data/talos/kubeconfig'",
            kubeconfig
        ));
    }
    
    // Check if the kubeconfig file exists
    if !std::path::Path::new(&kubeconfig).exists() {
        return Err(format!(
            "Kubeconfig file doesn't exist: {}. Is the Talos cluster running? Try: process-compose up",
            kubeconfig
        ));
    }
    
    Ok(())
}

/// Struct to manage test environment setup and teardown
pub struct TalosTestEnv {
    pub config: Arc<Config>,
    pub cleanup_on_drop: bool,
}

impl TalosTestEnv {
    pub async fn new() -> Result<Self, String> {
        validate_talos_environment()?;
        
        let config = get_test_config();
        
        // Ensure test namespace exists
        if let Err(e) = ensure_test_namespace(&config.workshop_namespace).await {
            eprintln!("Warning: Failed to ensure test namespace exists: {}", e);
        }
        
        Ok(Self {
            config,
            cleanup_on_drop: true,
        })
    }
    
    pub async fn with_cleanup(cleanup: bool) -> Result<Self, String> {
        let mut env = Self::new().await?;
        env.cleanup_on_drop = cleanup;
        Ok(env)
    }
}

impl Drop for TalosTestEnv {
    fn drop(&mut self) {
        if self.cleanup_on_drop {
            // Spawn a blocking task to clean up
            let namespace = self.config.workshop_namespace.clone();
            let workshop_name = self.config.workshop_name.clone();
            
            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    if let Err(e) = cleanup_test_resources(&namespace, &workshop_name).await {
                        eprintln!("Warning: Failed to cleanup test resources: {}", e);
                    }
                });
            });
        }
    }
}

/// Ensure the test namespace exists
async fn ensure_test_namespace(namespace: &str) -> Result<(), String> {
    use kube::{Api, Client};
    use k8s_openapi::api::core::v1::Namespace;
    
    let client = Client::try_default()
        .await
        .map_err(|e| format!("Failed to create Kubernetes client: {}", e))?;
    
    let ns_api: Api<Namespace> = Api::all(client);
    
    // Try to get the namespace
    match ns_api.get(namespace).await {
        Ok(_) => {
            println!("Test namespace '{}' already exists", namespace);
            Ok(())
        }
        Err(_) => {
            // Namespace doesn't exist, create it
            println!("Creating test namespace '{}'", namespace);
            
            let ns: Namespace = serde_json::from_value(serde_json::json!({
                "apiVersion": "v1",
                "kind": "Namespace",
                "metadata": {
                    "name": namespace
                }
            }))
            .map_err(|e| format!("Failed to construct namespace: {}", e))?;
            
            ns_api
                .create(&kube::api::PostParams::default(), &ns)
                .await
                .map_err(|e| format!("Failed to create namespace: {}", e))?;
            
            println!("Test namespace '{}' created", namespace);
            Ok(())
        }
    }
}

/// Clean up test resources
async fn cleanup_test_resources(namespace: &str, workshop_name: &str) -> Result<(), String> {
    use k8s_openapi::api::core::v1::{Pod, Service};
    use kube::{api::{DeleteParams, ListParams}, Api, Client};
    
    let client = Client::try_default()
        .await
        .map_err(|e| format!("Failed to create Kubernetes client: {}", e))?;
    
    let pod_api: Api<Pod> = Api::namespaced(client.clone(), namespace);
    let svc_api: Api<Service> = Api::namespaced(client, namespace);
    
    let label_selector = format!(
        "workshop-hub/workshop-name={},app.kubernetes.io/managed-by=workshop-hub",
        workshop_name
    );
    
    let list_params = ListParams::default().labels(&label_selector);
    
    // Delete all test pods
    if let Ok(pods) = pod_api.list(&list_params).await {
        for pod in pods.items {
            if let Some(name) = pod.metadata.name {
                let _ = pod_api.delete(&name, &DeleteParams::default()).await;
            }
        }
    }
    
    // Delete all test services
    if let Ok(services) = svc_api.list(&list_params).await {
        for service in services.items {
            if let Some(name) = service.metadata.name {
                let _ = svc_api.delete(&name, &DeleteParams::default()).await;
            }
        }
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_config_values() {
        let config = get_test_config();
        
        // Verify sensible defaults
        assert!(config.workshop_ttl_seconds > 0);
        assert!(config.workshop_idle_seconds > 0);
        assert!(config.workshop_pod_limit > 0);
        assert_eq!(config.workshop_namespace, "test-ns");
        
        // Verify resource limits are set
        assert!(!config.workshop_cpu_request.is_empty());
        assert!(!config.workshop_mem_request.is_empty());
    }
    
    #[test]
    fn test_gc_config_has_short_timeouts() {
        let config = get_gc_test_config();
        
        // GC tests should have much shorter timeouts
        assert!(config.workshop_ttl_seconds < 300);
        assert!(config.workshop_idle_seconds < 60);
    }
    
    #[test]
    fn test_stress_config_has_higher_limits() {
        let base_config = get_test_config();
        let stress_config = get_stress_test_config();
        
        // Stress tests should have higher pod limits
        assert!(stress_config.workshop_pod_limit >= base_config.workshop_pod_limit);
    }
}