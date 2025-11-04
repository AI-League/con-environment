use axum::{
    extract::Request,
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
    Json, Router,
};
use futures_util::future::BoxFuture;
use serde::{Deserialize, Serialize};
use std::task::{Context, Poll};
use tower::{Layer, Service};
use tower_sessions::{Expiry, MemoryStore, Session, SessionManagerLayer, cookie::time::Duration};

// Session keys
const USER_ID_KEY: &str = "user_id";
const USERNAME_KEY: &str = "username";

// Login/logout routes
pub fn auth_routes() -> Router<crate::AppState> {
    Router::new()
        .route("/login", get(login_page).post(handle_login))
        .route("/logout", post(handle_logout))
}

// Login page handler - serves HTML form
async fn login_page() -> impl IntoResponse {
    axum::response::Html(include_str!("default_index.html"))
}

#[derive(Debug, Deserialize)]
struct LoginRequest {
    username: String,
}

#[derive(Debug, Serialize)]
struct LoginResponse {
    success: bool,
    message: String,
    redirect: Option<String>,
}

// Handle login POST request
async fn handle_login(
    session: Session,
    Json(login_req): Json<LoginRequest>,
) -> impl IntoResponse {
    // Simple username-based authentication (no password)
    // Generate a user ID from the username
    let user_id = format!("user-{}", sanitize_username(&login_req.username));
    
    // Store user info in session
    if let Err(e) = session.insert(USER_ID_KEY, user_id.clone()).await {
        return Json(LoginResponse {
            success: false,
            message: format!("Session error: {}", e),
            redirect: None,
        });
    }
    
    if let Err(e) = session.insert(USERNAME_KEY, login_req.username.clone()).await {
        return Json(LoginResponse {
            success: false,
            message: format!("Session error: {}", e),
            redirect: None,
        });
    }
    
    Json(LoginResponse {
        success: true,
        message: "Login successful".to_string(),
        redirect: Some("/workshop".to_string()),
    })
}

// Sanitize username to create valid Kubernetes labels
fn sanitize_username(username: &str) -> String {
    username
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .collect::<String>()
        .to_lowercase()
}

// Handle logout
async fn handle_logout(session: Session) -> impl IntoResponse {
    session.flush().await;
    Redirect::to("/login")
}

// Create session manager layer
pub fn create_session_layer() -> SessionManagerLayer<MemoryStore> {
    let session_store = MemoryStore::default();
    
    SessionManagerLayer::new(session_store)
        .with_secure(false) // Set to true in production with HTTPS
        .with_expiry(Expiry::OnInactivity(Duration::hours(24)))
        .with_name("workshop_session")
        .with_path("/")
        .with_http_only(true)
        .with_same_site(tower_sessions::cookie::SameSite::Lax)
}

/// User identity extracted from session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserIdentity {
    pub user_id: String,
    pub username: String,
}

/// Authentication middleware using sessions
#[derive(Clone)]
pub struct SessionAuthLayer {}

impl<S: Clone> Layer<S> for SessionAuthLayer {
    type Service = SessionAuthService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        SessionAuthService { inner }
    }
}

pub struct SessionAuthService<S> {
    inner: S,
}

impl<S: Clone> Clone for SessionAuthService<S> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<B, S> Service<Request<B>> for SessionAuthService<S>
where
    S: Service<Request<B>, Response = Response> + Clone + Send + 'static,
    S::Response: IntoResponse + Send,
    S::Future: Send + 'static,
    B: Send + 'static,
{
    type Response = Response;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request<B>) -> Self::Future {
        let inner = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, inner);

        Box::pin(async move {
            let session = match request.extensions().get::<Session>() {
                Some(session) => session.clone(),
                None => panic!("Session not found - ensure SessionManagerLayer is applied"),
            };

            // Try to get user identity from session
            let user_id: Option<String> = session.get(USER_ID_KEY).await.unwrap_or(None);
            let username: Option<String> = session.get(USERNAME_KEY).await.unwrap_or(None);

            // Add identity to request extensions if found
            let (mut parts, body) = request.into_parts();
            if let (Some(user_id), Some(username)) = (user_id, username) {
                parts.extensions.insert(UserIdentity { user_id, username });
            }
            
            let request = Request::from_parts(parts, body);
            inner.call(request).await
        })
    }
}

/// Layer that enforces login for protected routes
#[derive(Clone)]
pub struct RequireAuthLayer {}

impl<S> Layer<S> for RequireAuthLayer {
    type Service = RequireAuthMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RequireAuthMiddleware { inner }
    }
}

pub struct RequireAuthMiddleware<S> {
    inner: S,
}

impl<S: Clone> Clone for RequireAuthMiddleware<S> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<S, B> Service<Request<B>> for RequireAuthMiddleware<S>
where
    S: Service<Request<B>, Response = Response> + Clone + Send + 'static,
    S::Future: Send + 'static,
    B: Send + 'static,
{
    type Response = Response;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<B>) -> Self::Future {
        let mut inner = self.inner.clone();

        Box::pin(async move {
            // Check if user is authenticated
            if req.extensions().get::<UserIdentity>().is_none() {
                // Redirect to login
                return Ok(Redirect::to("/login").into_response());
            }

            // User is authenticated, proceed with request
            inner.call(req).await
        })
    }
}