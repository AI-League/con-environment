use crate::{auth, get_user_id_from_claims, orchestrator, AppState, HubError};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{Request, StatusCode, Uri},
    response::{IntoResponse, Response},
    Extension,
};
use http_body_util::BodyExt;
//use tokio_util::io::ReaderStream;
use tracing::{info, warn};

/// Axum handler that performs auth and proxies HTTP requests.
#[axum::debug_handler]
pub async fn http_gateway_handler(
    State(state): State<AppState>,
    Path((workshop, user_id, path)): Path<(String, String, String)>,
    Extension(claims): Extension<auth::Claims>,
    request: Request<Body>,
) -> Result<Response, StatusCode> {
    // 1. Validate Workshop and User
    if workshop != state.config.workshop_name {
        return Err(StatusCode::NOT_FOUND); // Not the workshop we're configured for
    }
    if user_id != get_user_id_from_claims(&claims) {
        return Err(StatusCode::FORBIDDEN); // Token user doesn't match URL user
    }
    info!("HTTP: Auth successful for user {}", user_id);

    // 2. Get or Create Pod
    let config = state.config.clone();
    let binding = match orchestrator::get_or_create_pod(
        &state.kube_client,
        &user_id,
        config, // <-- Pass the whole config Arc
    )
    .await
    {
        Ok(binding) => binding,
        Err(HubError::PodLimitReached) => {
            warn!("HTTP: Pod limit reached. Denying user {}.", user_id);
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }
        Err(e) => {
            warn!("HTTP: Failed to get/create pod for {}: {}", user_id, e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // 3. Proxy the request
    info!("HTTP: Proxying to {}", binding.cluster_dns_name);

    let (mut parts, body) = request.into_parts();
    let body = http_body_util::Full::new(body.collect().await.unwrap().to_bytes());
    parts.uri = Uri::builder().scheme("http").authority(binding.cluster_dns_name).path_and_query(path).build().expect("valid uri");
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

