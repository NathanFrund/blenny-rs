// blenny/src/auth.rs
use axum::Router;
use axum::response::IntoResponse;
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

    /// Paths that should bypass authentication middleware.
    /// The framework has no defaults — every public path must be explicitly declared.
    fn public_paths(&self) -> Vec<&'static str> {
        vec![]
    }

    /// Apply protection layer to the router
    fn protect_router(&self, router: Router) -> Router;
}

/// Standard JWT validation middleware.
///
/// Reads `auth_public_paths` from `AppState` to determine which paths bypass
/// authentication. Extracts JWT from `blenny_token` cookie or `Authorization: Bearer`
/// header. Redirects to `/login` on failure.
pub async fn validate_token(
    mut req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let state = req
        .extensions()
        .get::<std::sync::Arc<crate::AppState>>()
        .expect("AppState missing in middleware")
        .clone();

    let path = req.uri().path();
    if state.auth_public_paths.iter().any(|p| path == p) {
        return next.run(req).await.into_response();
    }

    if let Some(user) = User::from_headers(req.headers(), &state.jwt_secret) {
        req.extensions_mut().insert(user);
        return next.run(req).await.into_response();
    }

    axum::response::Redirect::to("/login").into_response()
}

// ---------- Inventory registration ----------

pub struct AuthRegistration {
    pub name: &'static str,
    pub constructor: fn() -> Arc<dyn AuthProvider>,
}

inventory::collect!(AuthRegistration);
