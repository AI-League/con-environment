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
    let method = request.method().clone();
    let uri = request.uri().clone();
    
    tracing::info!(
        "🌐 HTTP request - user: {}, method: {}, uri: {}, path: {:?}",
        user_id.user_id,
        method,
        uri,
        path
    );

    let config = state.config.clone();
    
    tracing::debug!("Getting or creating pod for user: {}", user_id.user_id);
    let binding = match orchestrator::get_or_create_pod(
        &state.kube_client,
        &user_id.user_id,
        config,
    )
    .await
    {
        Ok(binding) => {
            tracing::info!(
                "✓ Pod binding obtained - pod: {}, service: {}, dns: {}",
                binding.pod_name,
                binding.service_name,
                binding.cluster_dns_name
            );
            binding
        }
        Err(HubError::PodLimitReached) => {
            tracing::warn!(
                "❌ Pod limit reached - denying user {}",
                user_id.user_id
            );
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }
        Err(e) => {
            tracing::error!(
                "❌ Failed to get/create pod for user {}: {}",
                user_id.user_id,
                e
            );
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let path = path.unwrap_or("/".to_string());
    
    tracing::debug!(
        "Proxying to {}:8888{} for user {}",
        binding.cluster_dns_name,
        path,
        user_id.user_id
    );

    let (mut parts, body) = request.into_parts();
    let body = http_body_util::Full::new(body.collect().await.unwrap().to_bytes());
    
    parts.uri = Uri::builder()
        .scheme("http")
        .authority(format!("{}:8888", binding.cluster_dns_name))
        .path_and_query(path.clone())
        .build()
        .expect("valid uri");
    
    let proxy_req = Request::from_parts(parts, body.into());

    tracing::trace!(
        "Sending proxy request - uri: {}, method: {}",
        proxy_req.uri(),
        proxy_req.method()
    );

    match state.http_client.request(proxy_req).await {
        Ok(proxy_res) => {
            let status = proxy_res.status();
            tracing::info!(
                "✓ Proxy response received - status: {}, user: {}",
                status,
                user_id.user_id
            );
            Ok(proxy_res.into_response())
        }
        Err(e) => {
            tracing::error!(
                "❌ Proxy request failed for user {} to {}: {}",
                user_id.user_id,
                binding.cluster_dns_name,
                e
            );
            Err(StatusCode::BAD_GATEWAY)
        }
    }
} 