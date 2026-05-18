use axum::{
    Extension, Router,
    extract::{Form, Query},
    response::{IntoResponse, Redirect},
    routing::{get, post},
};
use axum_extra::extract::cookie::Cookie;
use std::sync::Arc;

use crate::app_state::AppState;
use crate::auth::{AuthProvider, Claims};
use crate::{blenny_auth_provider, blenny_module};

#[derive(Default)]
#[blenny_module]
#[blenny_auth_provider]
pub struct AuthModule;

impl AuthProvider for AuthModule {
    fn auth_routes(&self) -> Router {
        Router::new()
            .route("/login", get(login_form))
            .route("/login", post(login_submit))
            .route("/logout", get(logout))
    }

    fn public_paths(&self) -> Vec<&'static str> {
        vec!["/login", "/logout"]
    }

    fn protect_router(&self, router: Router) -> Router {
        router.route_layer(axum::middleware::from_fn(crate::auth::validate_token))
    }
}

// ---------- handlers ----------

/// GET /login – shows the login form.
async fn login_form(
    Extension(state): Extension<Arc<AppState>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    use axum::response::Html;
    let mut ctx = tera::Context::new();
    if let Some(err) = params.get("error") {
        ctx.insert("error", err);
    }
    Html(
        state
            .conduit
            .render("auth/login", &ctx)
            .unwrap_or_else(|e| format!("Template error: {e}")),
    )
}

/// POST /login – processes credentials, sets JWT cookie, redirects.
async fn login_submit(
    Extension(state): Extension<Arc<AppState>>,
    Form(creds): Form<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let username = creds.get("username").map(|s| s.as_str()).unwrap_or("");
    let password = creds.get("password").map(|s| s.as_str()).unwrap_or("");

    if username == "admin" && password == "password" {
        let claims = Claims {
            sub: username.to_string(),
            exp: (chrono::Utc::now() + chrono::Duration::hours(24)).timestamp() as usize,
        };
        let token = jsonwebtoken::encode(
            &jsonwebtoken::Header::default(),
            &claims,
            &jsonwebtoken::EncodingKey::from_secret(state.jwt_secret.as_bytes()),
        )
        .unwrap();

        // Set JWT as a cookie
        let cookie = Cookie::build(("blenny_token", token))
            .path("/")
            .http_only(true)
            .secure(false) // set true in production
            .same_site(axum_extra::extract::cookie::SameSite::Strict)
            .max_age(time::Duration::hours(24))
            .build();

        // Publish a greeting to the dashboard topic
        let _ = state
            .hub
            .publish("dashboard.greeting", format!("User {username} logged in"));

        axum::response::Response::builder()
            .status(303)
            .header("location", "/dashboard")
            .header("set-cookie", cookie.to_string())
            .body(axum::body::Body::empty())
            .unwrap()
    } else {
        // Redirect back to login with error (simple: just use a query param)
        Redirect::to("/login?error=Invalid+credentials").into_response()
    }
}

/// GET /logout – clears the cookie and redirects home.
async fn logout() -> impl IntoResponse {
    axum::response::Response::builder()
        .status(303)
        .header("location", "/login")
        .header("set-cookie", "blenny_token=; Path=/")
        .body(axum::body::Body::empty())
        .unwrap()
}


