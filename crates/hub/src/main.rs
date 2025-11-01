use axum::{
    routing::{get, get_service, post},
    Router,
};
use k8s_openapi::api::core::v1::{Pod, Service};
use kube::Client;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tower_http::services::ServeDir;
use tracing::Level;

// Project modules
mod auth;
mod config; // <-- Add config module
mod error;
mod gc;
mod orchestrator;
mod proxy;

pub use error::HubError;

/// Global application state shared across all handlers.
#[derive(Clone)]
pub struct AppState {
    /// Client for talking to the Kubernetes API.
    kube_client: Client,
    /// Secrets for signing and validating JWTs.
    auth_keys: Arc<auth::AuthKeys>,
    /// HTTP client for proxying.
    http_client: hyper_util::client::legacy::Client<
        hyper_util::client::legacy::connect::HttpConnector,
        http_body_util::Full<hyper::body::Bytes>,
    >,
    /// Hub configuration
    config: Arc<config::Config>, // <-- Add config
}

#[tokio::main]
async fn main() {
    // Set up logging
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    tracing::info!("Starting Workshop Hub...");

    // --- 1. Initialize Kubernetes Client ---
    let kube_client = Client::try_default()
        .await
        .expect("Failed to create Kubernetes client. Is KUBECONFIG set?");

    // --- 2. Initialize Auth ---
    let auth_keys = Arc::new(auth::AuthKeys::new(b"my-super-secret-key"));

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
        auth_keys,
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
        let svc_api = kube::Api::<Service>::namespaced(
            gc_state.kube_client.clone(),
            &gc_state.config.workshop_namespace,
        );

        let mut interval = tokio::time::interval(Duration::from_secs(300)); // Every 5 mins
        loop {
            interval.tick().await;
            tracing::info!("GC: Running cleanup...");
            if let Err(e) = gc::cleanup_idle_pods(
                &pod_api,
                &svc_api,
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
        // Mock login to get a token.
        // POST /login with JSON `{"username": "my-user"}`
        .route("/login", post(auth::simple_login_handler))
        //
        // --- Workshop Routes ---
        // These routes are all protected by the auth middleware.
        .route(
            "/:workshop/:user_id/*path",
            get(proxy::http_gateway_handler),
        )
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::auth_middleware,
        ))
        // --- Fallback ---
        // Serves static files (e.g., your login page)
        .fallback_service(
            get_service(ServeDir::new("public")).handle_error(|err| async move {
                (
                    hyper::StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to serve static file: {}", err),
                )
            }),
        )
        .with_state(state);

    // --- 7. Run Server ---
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    tracing::info!("Hub listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}

/// Helper to get the user ID from JWT claims.
fn get_user_id_from_claims(claims: &auth::Claims) -> String {
    // Use a sanitized version of the username as the user ID
    // In a real app, this would be a stable database ID.
    claims
        .sub
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-')
        .collect::<String>()
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    pub mod gc;
    pub mod helpers;
    pub mod config;
    pub mod integration;
}