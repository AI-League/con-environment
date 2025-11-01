
use std::sync::Arc;
use crate::config::Config;

/// Environment validation for Talos test cluster
#[derive(Debug)]
pub struct TalosTestEnv {
    pub kubeconfig: String,
    pub cluster_name: String,
}

/// Validates that we're running in the correct test environment
pub fn validate_talos_environment() -> Result<TalosTestEnv, String> {
    // Check for KUBECONFIG
    let kubeconfig = std::env::var("KUBECONFIG")
        .map_err(|_| "KUBECONFIG not set. Please run: export KUBECONFIG=~/.kube/config")?;
    
    // Check if it points to a Talos config
    if !kubeconfig.contains("talos") && !kubeconfig.contains("test") {
        return Err(format!(
            "KUBECONFIG doesn't appear to be for a test cluster: {}. \
             For safety, tests only run against clusters with 'talos' or 'test' in the path.",
            kubeconfig
        ));
    }
    
    Ok(TalosTestEnv {
        kubeconfig,
        cluster_name: "talos-test".to_string(),
    })
}

/// Get base test configuration with reasonable defaults
pub fn get_test_config() -> Arc<Config> {
    Arc::new(Config {
        workshop_name: "test-workshop".to_string(),
        workshop_namespace: "test-workshops".to_string(),  // Cross-namespace: workshops go here
        workshop_ttl_seconds: 600,      // 10 minutes
        workshop_idle_seconds: 120,     // 2 minutes
        workshop_image: "nginxdemos/hello".to_string(),
        workshop_port: 80,
        workshop_pod_limit: 10,
        workshop_cpu_request: "50m".to_string(),
        workshop_cpu_limit: "200m".to_string(),
        workshop_mem_request: "64Mi".to_string(),
        workshop_mem_limit: "256Mi".to_string(),
    })
}

/// Get configuration optimized for GC tests (shorter timeouts)
pub fn get_gc_test_config() -> Arc<Config> {
    Arc::new(Config {
        workshop_name: "gc-test-workshop".to_string(),
        workshop_namespace: "test-workshops".to_string(),  // Cross-namespace
        workshop_ttl_seconds: 30,       // 30 seconds for quick TTL testing
        workshop_idle_seconds: 10,      // 10 seconds for quick idle testing
        workshop_image: "nginxdemos/hello".to_string(),
        workshop_port: 80,
        workshop_pod_limit: 5,
        workshop_cpu_request: "50m".to_string(),
        workshop_cpu_limit: "100m".to_string(),
        workshop_mem_request: "32Mi".to_string(),
        workshop_mem_limit: "128Mi".to_string(),
    })
}

/// Get configuration for stress testing (higher limits)
pub fn get_stress_test_config() -> Arc<Config> {
    Arc::new(Config {
        workshop_name: "stress-workshop".to_string(),
        workshop_namespace: "test-workshops".to_string(),  // Cross-namespace
        workshop_ttl_seconds: 1800,     // 30 minutes
        workshop_idle_seconds: 300,     // 5 minutes
        workshop_image: "nginxdemos/hello".to_string(),
        workshop_port: 80,
        workshop_pod_limit: 50,         // Much higher limit for stress testing
        workshop_cpu_request: "100m".to_string(),
        workshop_cpu_limit: "500m".to_string(),
        workshop_mem_request: "128Mi".to_string(),
        workshop_mem_limit: "512Mi".to_string(),
    })
}
