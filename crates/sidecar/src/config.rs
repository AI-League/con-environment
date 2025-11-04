use serde::Deserialize;
use std::fmt;

#[derive(Deserialize, Debug, Clone)]
pub struct Config {
    /// Address for the HTTP health server (e.g., "0.0.0.0:8080")
    pub http_listen: String,

    /// Address for the TCP proxy server (e.g., "0.0.0.0:8888")
    pub tcp_listen: String,

    /// Upstream target TCP address (e.g., "127.0.0.1:9000")
    pub target_tcp: Option<String>,

    /// Upstream target Unix Domain Socket path (e.g., "/var/run/app.sock")
    pub target_uds: Option<String>,
}

impl Config {
    /// Validates that exactly one target (TCP or UDS) is specified.
    pub fn validate(&self) -> Result<(), String> {
        match (&self.target_tcp, &self.target_uds) {
            (Some(_), Some(_)) => Err("Both SIDECAR_TARGET_TCP and SIDECAR_TARGET_UDS are set. Please specify only one.".to_string()),
            (None, None) => Err("No proxy target specified. Please set either SIDECAR_TARGET_TCP or SIDECAR_TARGET_UDS.".to_string()),
            _ => Ok(()),
        }
    }

    /// Loads configuration from environment variables.
    pub fn from_env() -> Result<Self, envy::Error> {
        for (key, value) in std::env::vars() {
            if key.starts_with("SIDECAR") {
                tracing::info!("Environment variable: {}={}", key, value);
            }
        }
        envy::prefixed("SIDECAR_").from_env::<Config>()
    }

}

impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "HTTP Listen: {}, TCP Listen: {}, Target: {}",
            self.http_listen,
            self.tcp_listen,
            self.target_tcp
                .as_deref()
                .unwrap_or_else(|| self.target_uds.as_deref().unwrap_or("None"))
        )
    }
}


#[cfg(test)]
mod tests {
    use super::*; // Import everything from the parent module (Config, etc.)
    use std::env;
    use std::sync::Mutex;

    // Mutex to ensure tests that modify the environment don't run concurrently
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[tracing_test::traced_test]
    #[test]
    fn test_load_http_listen_from_env() {
        // Acquire a lock to modify the global environment safely
        let _lock = ENV_LOCK.lock().unwrap();

        // --- Arrange ---
        // Set up a complete and valid set of environment variables
        // We must set all required fields (http_listen, tcp_listen)
        // and exactly one target field (target_tcp or target_uds).
        
        let expected_http_addr = "127.0.0.1:8080";
        let expected_tcp_addr = "0.0.0.0:8888";
        let expected_target = "10.0.0.1:9000";

        env::set_var("SIDECAR_HTTP_LISTEN", expected_http_addr);
        env::set_var("SIDECAR_TCP_LISTEN", expected_tcp_addr);
        env::set_var("SIDECAR_TARGET_TCP", expected_target);
        
        // Ensure the conflicting variable is not set
        env::remove_var("SIDECAR_TARGET_UDS");

        // --- Act ---
        // Call the function we are testing
        let config = Config::from_env().expect("Failed to load config from env");
        
        // We should also test our validation logic
        config.validate().expect("Config validation failed");

        // --- Assert ---
        // Check that the specific field (http_listen) is correct
        assert_eq!(config.http_listen, expected_http_addr);

        // We can also check that the other fields were loaded correctly
        assert_eq!(config.tcp_listen, expected_tcp_addr);
        assert_eq!(config.target_tcp, Some(expected_target.to_string()));
        assert_eq!(config.target_uds, None);
        
        // --- Cleanup ---
        // It's good practice to unset the variables after the test
        env::remove_var("SIDECAR_HTTP_LISTEN");
        env::remove_var("SIDECAR_TCP_LISTEN");
        env::remove_var("SIDECAR_TARGET_TCP");
    }
}