use serde::Deserialize;
use std::fmt;

#[derive(Deserialize, Debug, Clone)]
pub struct Config {
    /// Address for the HTTP health server (e.g., "0.0.0.0:8080")
    #[serde(rename = "HTTP_LISTEN")]
    pub http_listen_addr: String,

    /// Address for the TCP proxy server (e.g., "0.0.0.0:8888")
    #[serde(rename = "TCP_LISTEN")]
    pub tcp_listen_addr: String,

    /// Upstream target TCP address (e.g., "127.0.0.1:9000")
    #[serde(rename = "TARGET_TCP")]
    pub target_tcp_addr: Option<String>,

    /// Upstream target Unix Domain Socket path (e.g., "/var/run/app.sock")
    #[serde(rename = "TARGET_UDS")]
    pub target_uds_path: Option<String>,
}

impl Config {
    /// Validates that exactly one target (TCP or UDS) is specified.
    pub fn validate(&self) -> Result<(), String> {
        match (&self.target_tcp_addr, &self.target_uds_path) {
            (Some(_), Some(_)) => Err("Both SIDECAR_TARGET_TCP and SIDECAR_TARGET_UDS are set. Please specify only one.".to_string()),
            (None, None) => Err("No proxy target specified. Please set either SIDECAR_TARGET_TCP or SIDECAR_TARGET_UDS.".to_string()),
            _ => Ok(()),
        }
    }

    /// Loads configuration from environment variables.
    pub fn from_env() -> Result<Self, envy::Error> {
        envy::prefixed("SIDECAR_").from_env::<Config>()
    }

}

impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "HTTP Listen: {}, TCP Listen: {}, Target: {}",
            self.http_listen_addr,
            self.tcp_listen_addr,
            self.target_tcp_addr
                .as_deref()
                .unwrap_or_else(|| self.target_uds_path.as_deref().unwrap_or("None"))
        )
    }
}
