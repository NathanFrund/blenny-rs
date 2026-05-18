// blenny/src/auth.rs
use axum::Router;
use std::sync::Arc;

// ---------- JWT Claims & User ----------

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct Claims {
    pub sub: String, // user id
    pub exp: usize,  // expiry timestamp
}

#[derive(Debug, Clone)]
pub struct User {
    pub id: String,
}

impl User {
    /// Extract and validate a User from a raw JWT token string.
    pub fn from_token(token: &str, jwt_secret: &str) -> Option<Self> {
        let decoding_key = jsonwebtoken::DecodingKey::from_secret(jwt_secret.as_bytes());
        jsonwebtoken::decode::<Claims>(token, &decoding_key, &jsonwebtoken::Validation::default())
            .ok()
            .map(|data| User {
                id: data.claims.sub,
            })
    }

    /// Extract and validate a User from request headers, using the given JWT secret.
    pub fn from_headers(headers: &axum::http::HeaderMap, jwt_secret: &str) -> Option<Self> {
        let token = headers
            .get(axum::http::header::COOKIE)
            .and_then(|v| v.to_str().ok())
            .and_then(|cookies_str| {
                cookies_str.split("; ").find_map(|cookie| {
                    let (name, value) = cookie.split_once('=')?;
                    if name.trim() == "blenny_token" {
                        Some(value.trim().to_string())
                    } else {
                        None
                    }
                })
            })
            .or_else(|| {
                headers
                    .get(axum::http::header::AUTHORIZATION)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.strip_prefix("Bearer ").map(|t| t.to_string()))
            })?;

        Self::from_token(&token, jwt_secret)
    }
}

// ---------- AuthProvider trait ----------

pub trait AuthProvider: Send + Sync {
    /// Login/logout routes (unprotected)
    fn auth_routes(&self) -> Router;

    /// Apply protection layer to the router
    fn protect_router(&self, router: Router) -> Router;
}

// ---------- Inventory registration ----------

pub struct AuthRegistration {
    pub name: &'static str,
    pub constructor: fn() -> Arc<dyn AuthProvider>,
}

inventory::collect!(AuthRegistration);
