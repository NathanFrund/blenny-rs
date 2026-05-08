use crate::{BlennyModule, Conduit, blenny_module};
use axum::{Extension, Router, response::Html};
use std::sync::Arc;

#[derive(Default)]
#[blenny_module]
pub struct AuthModule;

impl BlennyModule for AuthModule {
    fn name(&self) -> &'static str {
        "Auth"
    }

    fn register_routes(&self, router: Router) -> Router {
        router.route("/login", axum::routing::get(login_form))
    }
}

async fn login_form(Extension(conduit): Extension<Arc<Conduit>>) -> Html<String> {
    let ctx = tera::Context::new();
    let html = conduit
        .render("auth/login", &ctx)
        .unwrap_or_else(|e| format!("Template error: {e}"));
    Html(html)
}
