use axum::{
    extract::{Query, State},
    http::{Request, StatusCode},
    response::{Json, Response},
};
use futures_util::future::BoxFuture;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tower::{Layer, Service};

use crate::AppState;

// --- 1. DATA STRUCTURES ---

/// JWT claims.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
    pub iat: usize,
}

/// Holds the secret keys for encoding/decoding JWTs.
#[derive(Clone)]
pub struct AuthKeys {
    pub encoding: EncodingKey,
    pub decoding: DecodingKey,
}

impl AuthKeys {
    pub fn new(secret: &[u8]) -> Self {
        Self {
            encoding: EncodingKey::from_secret(secret),
            decoding: DecodingKey::from_secret(secret),
        }
    }
}

/// Struct for parsing the `?token=...` query parameter
#[derive(Debug, Deserialize)]
struct TokenQuery {
    token: String,
}

/// Login request body from the login page.
#[derive(Deserialize)]
pub struct LoginRequest {
    username: String,
}

// --- 2. PUBLIC HANDLER ---

/// Simple login handler. Creates a JWT for a user.
#[axum::debug_handler]
pub async fn simple_login_handler(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<axum::Json<serde_json::Value>, axum::response::ErrorResponse> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let claims = Claims {
        sub: payload.username,
        // Token expires in 1 day
        exp: (now + Duration::from_secs(86400).as_secs()) as usize,
        iat: now as usize,
    };

    let token = encode(&Header::default(), &claims, &state.auth_keys.encoding)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to create token"))?;

    // Return the token AND the runtime workshop_name
    Ok(Json(json!({
        "token": token,
        "workshop_name": state.config.workshop_name
    })))
}

// --- 3. TOWER LAYER AND SERVICE (PASSIVE) ---

/// The Layer that applies our *passive* authentication Service.
#[derive(Clone)]
pub struct AuthLayer {
    auth_keys: Arc<AuthKeys>,
}

impl AuthLayer {
    pub fn new(auth_keys: Arc<AuthKeys>) -> Self {
        Self { auth_keys }
    }
}

impl<S> Layer<S> for AuthLayer {
    type Service = AuthService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        AuthService {
            inner,
            auth_keys: self.auth_keys.clone(),
        }
    }
}

/// The Service that *passively* checks for auth.
/// It injects `Option<Claims>` into the request extensions and *never* rejects.
#[derive(Clone)]
pub struct AuthService<S> {
    inner: S,
    auth_keys: Arc<AuthKeys>,
}

impl<S, B> Service<Request<B>> for AuthService<S>
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

    fn call(&mut self, mut req: Request<B>) -> Self::Future {
        let keys = self.auth_keys.clone();
        let mut inner = self.inner.clone();

        Box::pin(async move {
            // 1. Try to extract token
            if let Ok(token_str) = extract_token_from_request(&req) {
                // 2. If token exists, try to validate it
                if let Ok(claims) = validate_token(&keys, &token_str) {
                    // 3. If valid, inject claims
                    req.extensions_mut().insert(claims);
                }
            }
            // 4. Always call the inner service
            inner.call(req).await
        })
    }
}

// --- 4. HELPER FUNCTIONS ---

/// Extracts the JWT from header or query param.
pub(crate) fn extract_token_from_request<B>(request: &Request<B>) -> Result<String, &'static str> {
    // 1. Try Authorization header
    if let Some(auth_header) = request
        .headers()
        .get("Authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
    {
        return Ok(auth_header.to_string());
    }

    // 2. Try query parameter
    if let Some(query) = request.uri().query() {
        if let Ok(params) = serde_urlencoded::from_str::<TokenQuery>(query) {
            return Ok(params.token);
        }
    }
    Err("No token found in header or query")
}

/// Validates the JWT string and returns the claims.
fn validate_token(keys: &AuthKeys, token: &str) -> Result<Claims, &'static str> {
    decode::<Claims>(token, &keys.decoding, &Validation::default())
        .map(|data| data.claims)
        .map_err(|_err| "Invalid token")
}