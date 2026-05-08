use crate::{BlennyModule, Conduit, TransportHub, blenny_module};
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

async fn login_form(
    Extension(conduit): Extension<Arc<Conduit>>,
    Extension(hub): Extension<Arc<TransportHub>>,
) -> Html<String> {
    // Broadcast a live-update message.
    hub.broadcast_html("<p>Someone visited the login page!</p>");

    let mut ctx = tera::Context::new();
    ctx.insert("username", "Nathan");
    let html = conduit
        .render("auth/login", &ctx)
        .unwrap_or_else(|e| format!("Template error: {e}"));
    Html(html)
}
