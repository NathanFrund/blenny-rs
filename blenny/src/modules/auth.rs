use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Json, Router};

use crate::auth::{AuthProvider, Claims, User};
use crate::{blenny_auth_provider, blenny_module, BlennyModule};

#[derive(Default)]
#[blenny_module]
#[blenny_auth_provider]
pub struct AuthModule;

impl BlennyModule for AuthModule {
    fn name(&self) -> &'static str { "Auth" }
    fn register_routes(&self, router: Router) -> Router {
        // No extra routes; auth routes come from AuthProvider.
        router
    }
}

impl AuthProvider for AuthModule {
    fn auth_routes(&self) -> Router {
        Router::new()
            .route("/login", axum::routing::post(login_submit))
    }

    fn protect_router(&self, router: Router) -> Router {
        router.layer(
            tower::ServiceBuilder::new()
                .layer(axum::middleware::from_fn(validate_token))
        )
    }
}

// ---------- handlers ----------

async fn login_submit(Json(creds): Json<serde_json::Value>) -> impl IntoResponse {
    let username = creds["username"].as_str().unwrap_or("");
    let password = creds["password"].as_str().unwrap_or("");
    if username == "admin" && password == "password" {
        let claims = Claims {
            sub: username.to_string(),
            exp: (chrono::Utc::now() + chrono::Duration::hours(24)).timestamp() as usize,
        };
        let token = jsonwebtoken::encode(
            &jsonwebtoken::Header::default(),
            &claims,
            &jsonwebtoken::EncodingKey::from_secret(b"my-secret-key"),
        )
        .unwrap();
        (StatusCode::OK, axum::Json(serde_json::json!({ "token": token }))).into_response()
    } else {
        (StatusCode::UNAUTHORIZED, "Invalid credentials").into_response()
    }
}

async fn validate_token(
    mut req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> impl IntoResponse {
    let header = req.headers().get("Authorization").and_then(|v| v.to_str().ok());
    if let Some(header) = header {
        if let Some(token) = header.strip_prefix("Bearer ") {
            let decoding_key = jsonwebtoken::DecodingKey::from_secret(b"my-secret-key");
            if let Ok(token_data) = jsonwebtoken::decode::<Claims>(
                token,
                &decoding_key,
                &jsonwebtoken::Validation::default(),
            ) {
                req.extensions_mut().insert(User {
                    id: token_data.claims.sub,
                });
                return next.run(req).await;
            }
        }
    }
    // HTMX‑aware redirect
    if req.headers().contains_key("HX-Request") {
        return (
            StatusCode::UNAUTHORIZED,
            [("HX-Redirect", "/login")],
            "Unauthorized",
        ).into_response();
    }
    (StatusCode::UNAUTHORIZED, "Missing or invalid token").into_response()
}
