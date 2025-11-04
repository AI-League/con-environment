use axum::{
    Router, response::{Html, IntoResponse, Response}, routing::{get, post}
};
use hyper::StatusCode;
use k8s_openapi::api::core::v1::Pod;
use kube::Client;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tower_http::trace::TraceLayer;
use tower_cookies::CookieManagerLayer;

// Project modules
mod auth;
mod config; // <-- Add config module
mod error;
mod gc;
mod orchestrator;
mod proxy;

pub use error::HubError;

use crate::{proxy::{workshop_index_handler, workshop_other_handler}};

pub static SIDECAR: &'static str = "ghcr.io/nbhdai/workshop-sidecar:latest";

/// Global application state shared across all handlers.
#[derive(Clone)]
pub struct AppState {
    /// Client for talking to the Kubernetes API.
    kube_client: Client,
    /// HTTP client for proxying.
    http_client: hyper_util::client::legacy::Client<
        hyper_util::client::legacy::connect::HttpConnector,
        http_body_util::Full<hyper::body::Bytes>,
    >,
    /// Hub configuration
    config: Arc<config::Config>, // <-- Add config
}

async fn index() -> Result<Response, StatusCode> {
    return Ok(Html(include_str!("default_index.html")).into_response());
}

#[tokio::main]
async fn main() {
    // Set up logging
    tracing_subscriber::registry()
        .with(fmt::layer().with_target(true))
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            "trace,tower_http=trace,fred=debug,h2=off,hyper=off,sqlx=off,tarpc=off,rustls=off".into()
        }))
        .init();

    tracing::info!("Starting Workshop Hub...");

    // --- 1. Initialize Kubernetes Client ---
    let kube_client = Client::try_default()
        .await
        .expect("Failed to create Kubernetes client. Is KUBECONFIG set?");

    // --- 2. Initialize Auth ---
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://workshop-redis.workshop-system.svc.cluster.local:6379".to_string());
    

    // --- 3. Initialize Config ---
    let config = Arc::new(config::Config::from_env().expect("Failed to load config from env"));
    tracing::info!("Config loaded: {:?}", config);

    // --- 4. Initialize HTTP Proxy Client ---
    let http_client =
        hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
            .build_http();

    // --- 5. Create AppState ---
    let state = AppState {
        kube_client: kube_client.clone(),
        http_client,
        config: config.clone(), // <-- Add config to state
    };

    // --- 6. Spawn Garbage Collector ---
    let gc_state = state.clone();
    tokio::spawn(async move {
        tracing::info!("Spawning Garbage Collector task.");
        // Use the configured namespace for the GC
        let pod_api = kube::Api::<Pod>::namespaced(
            gc_state.kube_client.clone(),
            &gc_state.config.workshop_namespace,
        );

        let mut interval = tokio::time::interval(Duration::from_secs(300)); // Every 5 mins
        loop {
            interval.tick().await;
            tracing::info!("GC: Running cleanup...");
            if let Err(e) = gc::cleanup_idle_pods(
                &pod_api,
                &gc_state.config.workshop_name,
                gc_state.config.workshop_idle_seconds,
            )
            .await
            {
                tracing::error!("GC: Error during cleanup: {}", e);
            }
        }
    });

    // --- 7. Define Routes ---
    let app = Router::new()
        .route("/workshop/", get(workshop_index_handler))
        .route("/workshop/{*path}", get(workshop_other_handler))
        // Apply auth requirement ONLY to these routes
        .layer(auth::RequireAuthLayer {})
        .route("/", get(index))
                // Apply middleware layers (order matters!)
        .merge(auth::auth_routes())
        .layer(auth::CookieAuthLayer {})
        .layer(CookieManagerLayer::new())
        .route("/health", get(|| async { "OK" }))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    // --- 7. Run Server ---
    let addr = SocketAddr::from(([0; 8], 8080));
    tracing::info!("Hub listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}

// #[cfg(test)]
// mod tests {
//     pub mod gc;
//     pub mod helpers;
//     pub mod config;
//     pub mod integration;
// }