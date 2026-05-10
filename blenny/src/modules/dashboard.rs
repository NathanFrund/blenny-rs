use axum::{Extension, Router, response::Html};
use crate::{blenny_module, BlennyModule, AppState, auth::User};
use std::sync::Arc;

#[derive(Default)]
#[blenny_module]
pub struct DashboardModule;

impl BlennyModule for DashboardModule {
    fn name(&self) -> &'static str { "Dashboard" }
    fn register_routes(&self, router: Router) -> Router {
        router.route("/dashboard", axum::routing::get(dashboard_handler))
    }
}

async fn dashboard_handler(
    Extension(state): Extension<Arc<AppState>>,
    Extension(user): Extension<User>,
) -> Html<String> {
    let conduit = state.conduit.as_ref().expect("Conduit not set");
    let mut ctx = tera::Context::new();
    ctx.insert("username", &user.id);
    let html = conduit
        .render("auth/login", &ctx)
        .unwrap_or_else(|e| format!("Template error: {e}"));
    Html(html)
}