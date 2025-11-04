use axum::extract::State;
use axum::{routing::get, Json, Router};
use serde::Serialize;
use tower_http::trace::TraceLayer;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{config::Config, AppState};

#[derive(Serialize)]
struct HealthStatus {
    status: String,
    last_activity_timestamp: i64,
    idle_seconds: u64,
}

/// Runs the Axum HTTP server for health checks.
pub async fn run_http_server(
    state: Arc<AppState>,
    config: Arc<Config>,
) -> Result<(), std::io::Error> {
    let app = Router::new()
        .route("/health", get(health_handler))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&config.http_listen_addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

/// Responds with the current activity status.
async fn health_handler(State(state): State<Arc<AppState>>) -> Json<HealthStatus> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let last_activity = state.get_last_activity();
    let idle_seconds = (now - last_activity).max(0) as u64;

    Json(HealthStatus {
        status: "ok".to_string(),
        last_activity_timestamp: last_activity,
        idle_seconds,
    })
}
