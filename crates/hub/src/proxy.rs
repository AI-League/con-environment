use crate::{AppState, HubError, auth::UserIdentity, orchestrator};
use axum::{
    Extension, body::Body, extract::{Path, State}, http::{Request, StatusCode, Uri}, response::{IntoResponse, Response}
};
use http_body_util::BodyExt;
//use tokio_util::io::ReaderStream;
use tracing::{info, debug, warn};

#[axum::debug_handler]
pub async fn workshop_index_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<UserIdentity>,
    request: Request<Body>,
) -> Result<Response, StatusCode> {
    http_handler(state, None, claims, request).await
}

/// Axum handler that performs auth and proxies HTTP requests.
#[axum::debug_handler]
pub async fn workshop_other_handler(
    State(state): State<AppState>,
    Path(path): Path<String>,
    Extension(claims): Extension<UserIdentity>,
    request: Request<Body>,
) -> Result<Response, StatusCode> {
    http_handler(state, Some(path), claims, request).await
}

pub async fn http_handler(
    state: AppState,
    path: Option<String>,
    user_id: UserIdentity,
    request: Request<Body>,
) -> Result<Response, StatusCode> {
    info!("HTTP: Auth successful for user {}", user_id.user_id);

    // 2. Get or Create Pod
    let config = state.config.clone();
    let binding = match orchestrator::get_or_create_pod(
        &state.kube_client,
        &user_id.user_id,
        config, // <-- Pass the whole config Arc
    )
    .await
    {
        Ok(binding) => binding,
        Err(HubError::PodLimitReached) => {
            warn!("HTTP: Pod limit reached. Denying user {}.", user_id.user_id);
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }
        Err(e) => {
            warn!("HTTP: Failed to get/create pod for {}: {}", user_id.user_id, e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // 3. Proxy the request
    info!("HTTP: Proxying to {}:8888", binding.cluster_dns_name);
    let path = path.unwrap_or("/".to_string());

    let (mut parts, body) = request.into_parts();
    let body = http_body_util::Full::new(body.collect().await.unwrap().to_bytes());
    
    // Build URI with explicit port 8888 (the sidecar's proxy port)
    parts.uri = Uri::builder()
        .scheme("http")
        .authority(format!("{}:8888", binding.cluster_dns_name))
        .path_and_query(path)
        .build()
        .expect("valid uri");
    
    let proxy_req = Request::from_parts(parts, body.into());

    // Send the proxy request
    match state.http_client.request(proxy_req).await {
        Ok(proxy_res) => Ok(proxy_res.into_response()),
        Err(e) => {
            warn!("HTTP: Proxy request failed: {}", e);
            Err(StatusCode::BAD_GATEWAY)
        }
    }
}
