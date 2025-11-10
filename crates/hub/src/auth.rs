use axum::{
    extract::Request,
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
    Json, Router,
};
use futures_util::future::BoxFuture;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, trace};
use std::task::{Context, Poll};
use tower::{Layer, Service};
use tower_cookies::{Cookie, Cookies};
use chrono::{Duration, Utc};

const JWT_SECRET: &[u8] = b"your-secret-key-change-in-production"; // TODO: Load from env
const COOKIE_NAME: &str = "workshop_token";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Claims {
    sub: String, // user_id
    username: String,
    exp: i64,
}

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
    cookies: Cookies,
    Json(login_req): Json<LoginRequest>,
) -> impl IntoResponse {
    tracing::info!(
        "🔐 Login attempt for username: '{}' from IP: [extract from request if available]",
        login_req.username
    );
    
    // Check if there's already a cookie
    if let Some(old_cookie) = cookies.get(COOKIE_NAME) {
        tracing::debug!("Found existing cookie during login, will be replaced");
        cookies.remove(Cookie::from(COOKIE_NAME));
    }
    
    let user_id = format!("user-{}", sanitize_username(&login_req.username));
    
    let expiration = Utc::now() + Duration::hours(24);
    let claims = Claims {
        sub: user_id.clone(),
        username: login_req.username.clone(),
        exp: expiration.timestamp(),
    };
    
    tracing::debug!(
        "Creating JWT for user_id: {}, expires at: {}",
        user_id,
        expiration
    );
    
    let token = match encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(JWT_SECRET),
    ) {
        Ok(t) => {
            tracing::trace!("JWT created successfully (token length: {})", t.len());
            t
        }
        Err(e) => {
            tracing::error!("❌ Failed to create JWT: {}", e);
            return Json(LoginResponse {
                success: false,
                message: "Authentication error".to_string(),
                redirect: None,
            });
        }
    };
    
    let mut cookie = Cookie::new(COOKIE_NAME, token);
    cookie.set_http_only(true);
    cookie.set_same_site(tower_cookies::cookie::SameSite::Lax);
    cookie.set_path("/");
    cookie.set_max_age(tower_cookies::cookie::time::Duration::hours(24));
    
    tracing::debug!("Setting cookie with max_age: 24 hours");
    cookies.add(cookie);
    
    tracing::info!(
        "✅ Login successful - user_id: {}, username: {}",
        user_id,
        login_req.username
    );
    
    Json(LoginResponse {
        success: true,
        message: "Login successful".to_string(),
        redirect: Some("/workshop/".to_string()),
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
async fn handle_logout(cookies: Cookies) -> impl IntoResponse {
    tracing::info!("Logout request");
    cookies.remove(Cookie::from(COOKIE_NAME));
    Redirect::to("/login")
}

/// User identity extracted from JWT
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserIdentity {
    pub user_id: String,
    pub username: String,
}

/// Authentication middleware using JWT cookies
#[derive(Clone)]
pub struct CookieAuthLayer {}

impl<S: Clone> Layer<S> for CookieAuthLayer {
    type Service = CookieAuthService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        CookieAuthService { inner }
    }
}

pub struct CookieAuthService<S> {
    inner: S,
}

impl<S: Clone> Clone for CookieAuthService<S> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<B, S> Service<Request<B>> for CookieAuthService<S>
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
            let cookies = match request.extensions().get::<Cookies>() {
                Some(cookies) => cookies.clone(),
                None => {
                    tracing::error!("Cookies extension not found - ensure CookieManagerLayer is applied");
                    panic!("Cookies not found - ensure CookieManagerLayer is applied");
                }
            };

            let (mut parts, body) = request.into_parts();
            
            // ADD EXTENSIVE TRACING
            tracing::debug!(
                "CookieAuthService processing request to: {} {}", 
                parts.method, 
                parts.uri
            );
            
            if let Some(cookie) = cookies.get(COOKIE_NAME) {
                let token = cookie.value();
                tracing::trace!("Found JWT cookie, attempting validation");
                
                match decode::<Claims>(
                    token,
                    &DecodingKey::from_secret(JWT_SECRET),
                    &Validation::default(),
                ) {
                    Ok(token_data) => {
                        let claims = token_data.claims;
                        tracing::info!(
                            "✓ JWT validated successfully - user_id: {}, username: {}, expires: {}",
                            claims.sub,
                            claims.username,
                            claims.exp
                        );
                        parts.extensions.insert(UserIdentity {
                            user_id: claims.sub,
                            username: claims.username,
                        });
                    }
                    Err(e) => {
                        tracing::warn!(
                            "✗ Invalid JWT token: {} - Clearing bad cookie from client", 
                            e
                        );
                        
                        // CRITICAL FIX: Clear the bad cookie immediately
                        cookies.remove(Cookie::from(COOKIE_NAME));
                        
                        // If this is a protected route request, return early with redirect
                        // (The RequireAuthLayer will catch this on protected routes anyway)
                        tracing::debug!("Bad cookie cleared, request will proceed without auth");
                    }
                }
            } else {
                tracing::trace!("No JWT cookie found in request");
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
                tracing::warn!("Unauthenticated request to protected route, redirecting to login");
                return Ok(Redirect::to("/login").into_response());
            }

            tracing::debug!("Authenticated request proceeding");
            inner.call(req).await
        })
    }
}