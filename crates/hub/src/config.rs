use serde::Deserialize;

/// Top-level configuration loaded from environment variables.
#[derive(Deserialize, Debug, Clone)]
pub struct Config {
    /// The public-facing name for this set of workshops.
    #[serde(default = "default_workshop_name")]
    pub workshop_name: String,

    /// Namespace where workshop pods and services will be created.
    #[serde(default = "default_workshop_namespace")]
    pub workshop_namespace: String,

    /// Max time-to-live for a pod in seconds, regardless of activity.
    #[serde(default = "default_workshop_ttl")]
    pub workshop_ttl_seconds: u64,

    /// Max idle time in seconds before a pod is cleaned up.
    #[serde(default = "default_workshop_idle")]
    pub workshop_idle_seconds: u64,

    // --- New Fields Below ---

    /// The container image to use for the workshop.
    #[serde(default = "default_workshop_image")]
    pub workshop_image: String,

    /// The internal port the workshop container listens on.
    #[serde(default = "default_workshop_port")]
    pub workshop_port: u16,

    /// Max number of concurrent workshop pods allowed to run.
    #[serde(default = "default_workshop_pod_limit")]
    pub workshop_pod_limit: usize,

    /// Workshop container CPU request.
    #[serde(default = "default_workshop_cpu_request")]
    pub workshop_cpu_request: String,

    /// Workshop container CPU limit.
    #[serde(default = "default_workshop_cpu_limit")]
    pub workshop_cpu_limit: String,

    /// Workshop container memory request.
    #[serde(default = "default_workshop_mem_request")]
    pub workshop_mem_request: String,

    /// Workshop container memory limit.
    #[serde(default = "default_workshop_mem_limit")]
    pub workshop_mem_limit: String,
}

fn default_workshop_name() -> String { "workshop".to_string() }
fn default_workshop_namespace() -> String { "default".to_string() }
fn default_workshop_ttl() -> u64 { 8 * 60 * 60 } // 8 hours
fn default_workshop_idle() -> u64 { 60 * 60 } // 1 hour

// --- New Defaults Below ---
fn default_workshop_image() -> String { "nginx".to_string() } // Default to nginx
fn default_workshop_port() -> u16 { 80 }
fn default_workshop_pod_limit() -> usize { 100 } // Default to 100 pods
fn default_workshop_cpu_request() -> String { "100m".to_string() }
fn default_workshop_cpu_limit() -> String { "500m".to_string() }
fn default_workshop_mem_request() -> String { "128Mi".to_string() }
fn default_workshop_mem_limit() -> String { "512Mi".to_string() }


/// The annotation key we use to store the expiration time on a pod.
pub const TTL_ANNOTATION: &str = "workshop-hub/ttl-expires-at";
/// The label key for the workshop name.
pub const LABEL_WORKSHOP_NAME: &str = "workshop-hub/workshop-name";

impl Config {
    /// Loads configuration from environment variables.
    pub fn from_env() -> Result<Self, envy::Error> {
        envy::prefixed("HUB_").from_env::<Config>()
    }
}


