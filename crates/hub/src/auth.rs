use axum::{
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::{Json, Response},
};
use hyper::HeaderMap;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::AppState;

/// JWT claims. `sub` (subject) will be the username.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
    pub iat: usize,
}

/// Holds the secret keys for encoding/decoding JWTs.
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

/// Mock login request body.
#[derive(Deserialize)]
pub struct LoginRequest {
    username: String,
}

/// Simple login handler. Creates a JWT for a user with just their username. 
/// Insecure, but fine for workshops.
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

    Ok(Json(json!({ "token": token })))
}

/// Extracts the JWT from the Authorization header.
pub fn extract_token_from_headers(headers: &HeaderMap) -> Result<String, &'static str> {
    headers
        .get("Authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(|token| token.to_owned())
        .ok_or("Missing or invalid Authorization header")
}

/// Extracts the JWT from query parameters (e.g., ?token=...)
pub fn extract_token_from_query(query: &str) -> Result<String, &'static str> {
    query
        .split('&')
        .find_map(|pair| pair.strip_prefix("token="))
        .map(|token| token.to_owned())
        .ok_or("Missing token in query parameters")
}

/// Validates the JWT and returns the claims.
pub fn validate_token(keys: &AuthKeys, token: &str) -> Result<Claims, &'static str> {
    decode::<Claims>(token, &keys.decoding, &Validation::default())
        .map(|data| data.claims)
        .map_err(|_err| "Invalid token")
}

/// Axum middleware for authenticating a request.
pub async fn auth_middleware<B>(
    State(state): State<AppState>,
    mut request: Request<B>,
    next: Next<B>,
) -> Result<Response, StatusCode> {
    // Try extracting token from headers first
    let token = match extract_token_from_headers(request.headers()) {
        Ok(token) => token,
        // If not in headers, try query parameters (for WebSockets)
        Err(_) => {
            let query = request.uri().query().unwrap_or_default();
            match extract_token_from_query(query) {
                Ok(token) => token,
                Err(_) => return Err(StatusCode::UNAUTHORIZED),
            }
        }
    };

    let claims = match validate_token(&state.auth_keys, &token) {
        Ok(claims) => claims,
        Err(_) => return Err(StatusCode::UNAUTHORIZED),
    };

    // Store the claims in the request extensions
    // so the downstream handlers can access them.
    request.extensions_mut().insert(claims);

    // Continue to the next handler
    Ok(next.run(request).await)
}

