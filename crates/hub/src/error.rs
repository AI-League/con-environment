use kube::Error as KubeError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum HubError {
    #[error("Kubernetes API error: {0}")]
    KubeError(#[from] KubeError),

    #[error("Pod failed to become ready in time")]
    PodNotReady,

    #[error("Authentication failed")]
    AuthError,

    #[error("Internal proxy error: {0}")]
    ProxyError(String),

    #[error("Global pod limit reached")]
    PodLimitReached,

    #[error("Internal error: {0}")]
    InternalError(String),
}

// We can implement IntoResponse for our error
// (Not fully done here, but shows the idea)
impl axum::response::IntoResponse for HubError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            HubError::KubeError(_) => (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            ),
            HubError::PodNotReady => (
                axum::http::StatusCode::GATEWAY_TIMEOUT,
                "Workshop failed to start".to_string(),
            ),
            HubError::AuthError => (
                axum::http::StatusCode::UNAUTHORIZED,
                "Unauthorized".to_string(),
            ),
            HubError::ProxyError(msg) => (
                axum::http::StatusCode::BAD_GATEWAY,
                format!("Proxy error: {}", msg),
            ),
            HubError::PodLimitReached => (
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "Service is at capacity, please try again later".to_string(),
            ),
            HubError::InternalError(msg) => (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Internal error: {}", msg),
            ),
        };
        (status, message).into_response()
    }
}


