use axum::{
    Extension, Router,
    extract::{Form, Query},
    http::{StatusCode, header},
    response::{IntoResponse, Redirect},
    routing::{get, post},
};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use std::sync::Arc;

use crate::app_state::AppState;
use crate::auth::{AuthProvider, Claims, User};
use crate::{BlennyModule, blenny_auth_provider, blenny_module};

#[derive(Default)]
#[blenny_module]
#[blenny_auth_provider]
pub struct AuthModule;

impl BlennyModule for AuthModule {
    fn name(&self) -> &'static str {
        "Auth"
    }
    fn register_routes(&self, router: Router) -> Router {
        // No extra routes; auth routes come from AuthProvider.
        router
    }
}

impl AuthProvider for AuthModule {
    fn auth_routes(&self) -> Router {
        Router::new()
            .route("/login", get(login_form))
            .route("/login", post(login_submit))
            .route("/logout", get(logout))
    }

    fn protect_router(&self, router: Router) -> Router {
        router.layer(tower::ServiceBuilder::new().layer(axum::middleware::from_fn(validate_token)))
    }
}

// ---------- handlers ----------

/// GET /login – shows the login form.
async fn login_form(
    Extension(state): Extension<Arc<AppState>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    use axum::response::Html;
    let conduit = state.conduit.as_ref().expect("Conduit not set");
    let mut ctx = tera::Context::new();
    if let Some(err) = params.get("error") {
        ctx.insert("error", err);
    }
    Html(
        conduit
            .render("auth/login", &ctx)
            .unwrap_or_else(|e| format!("Template error: {e}")),
    )
}

/// POST /login – processes credentials, sets JWT cookie, redirects.
async fn login_submit(
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
            &jsonwebtoken::EncodingKey::from_secret(b"my-secret-key"),
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

        let mut response = Redirect::to("/dashboard").into_response();
        response
            .headers_mut()
            .insert("Set-Cookie", cookie.to_string().parse().unwrap());
        response
    } else {
        // Redirect back to login with error (simple: just use a query param)
        Redirect::to("/login?error=Invalid+credentials").into_response()
    }
}

/// GET /logout – clears the cookie and redirects home.
async fn logout() -> impl IntoResponse {
    let cookie = Cookie::build(("blenny_token", ""))
        .path("/")
        .max_age(time::Duration::seconds(0))
        .build();
    let mut response = Redirect::to("/login").into_response();
    response
        .headers_mut()
        .insert("Set-Cookie", cookie.to_string().parse().unwrap());
    response
}

/// Middleware that protects routes. Reads JWT from cookie or Authorization header.
async fn validate_token(
    jar: CookieJar,
    mut req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> impl IntoResponse {
    // Try cookie first
    let token_from_cookie = jar.get("blenny_token").map(|c| c.value().to_string());

    let token_from_header = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|val| val.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer ").map(|t| t.to_string()));

    let token = token_from_cookie.or(token_from_header);

    if let Some(token) = token {
        let decoding_key = jsonwebtoken::DecodingKey::from_secret(b"my-secret-key");
        if let Ok(token_data) = jsonwebtoken::decode::<Claims>(
            &token,
            &decoding_key,
            &jsonwebtoken::Validation::default(),
        ) {
            req.extensions_mut().insert(User {
                id: token_data.claims.sub,
            });
            return next.run(req).await;
        }
    }

    // Not authenticated.
    // If HTMX request, use HX-Redirect; otherwise full redirect.
    let is_htmx = req.headers().contains_key("HX-Request");
    if is_htmx {
        (StatusCode::UNAUTHORIZED, [("HX-Redirect", "/login")]).into_response()
    } else {
        // Clear potentially bad cookie and redirect
        let cookie = Cookie::build(("blenny_token", ""))
            .path("/")
            .max_age(time::Duration::seconds(0))
            .build();
        let mut response = Redirect::to("/login").into_response();
        response
            .headers_mut()
            .insert("Set-Cookie", cookie.to_string().parse().unwrap());
        response
    }
}
