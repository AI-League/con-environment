use std::sync::{atomic::{AtomicI64, Ordering}, Arc};
use std::time::{SystemTime, UNIX_EPOCH};

mod config;
mod http_server;
mod proxy; // <-- Renamed from tcp_proxy

use config::Config;
use tracing::{error, info};

/// Shared state between the HTTP server and the TCP proxy.
#[derive(Debug)]
pub struct AppState {
    /// The last time any activity was detected on a proxied stream.
    /// Stored as a Unix timestamp (seconds).
    last_activity: AtomicI64,
    
    // We don't really need this mutex, AtomicI64 is sufficient.
    // Keeping it simple.
}

impl AppState {
    fn new() -> Self {
        Self {
            last_activity: AtomicI64::new(current_timestamp()),
        }
    }

    /// Update the last activity timestamp to "now".
    pub fn update_activity(&self) {
        self.last_activity.store(current_timestamp(), Ordering::Relaxed);
    }

    /// Get the last activity timestamp.
    pub fn get_last_activity(&self) -> i64 {
        self.last_activity.load(Ordering::Relaxed)
    }
}

fn current_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("Starting workshop sidecar...");

    // 1. Load configuration
    let config = match envy::from_env::<Config>() {
        Ok(config) => config,
        Err(e) => {
            error!("Failed to load configuration: {}", e);
            std::process::exit(1);
        }
    };
    
    if let Err(e) = config.validate() {
        error!("Invalid configuration: {}", e);
        std::process::exit(1);
    }
    
    let config = Arc::new(config);
    info!("Configuration loaded: {:?}", config);

    // 2. Create shared state
    let state = Arc::new(AppState::new());

    // 3. Spawn the HTTP health server
    let http_state = state.clone();
    let http_config = config.clone();
    tokio::spawn(async move {
        info!("Starting HTTP health server...");
        if let Err(e) = http_server::run_http_server(http_state, http_config).await {
            error!("HTTP health server failed: {}", e);
        }
    });

    // 4. Run the TCP proxy server (blocking)
    info!("Starting TCP proxy server...");
    if let Err(e) = proxy::run_proxy(state, config).await {
        error!("TCP proxy server failed: {}", e);
    }
}

