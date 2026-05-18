use crate::{AppState, auth::User, blenny_module};
use axum::{Extension, Router, response::Html};
use std::sync::Arc;

#[derive(Default)]
#[blenny_module(
    route_handler = "dashboard_routes",
    initialize_handler = "initialize_module"
)]
pub struct DashboardModule;

impl DashboardModule {
    fn dashboard_routes(router: Router) -> Router {
        router.route("/dashboard", axum::routing::get(dashboard_handler))
    }

    fn initialize_module(&mut self, state: Arc<AppState>) {
        let mut rx = state
            .hub
            .subscribe_topic("some_topic")
            .expect("Failed to subscribe to topic");
        tokio::spawn(async move {
            while let Ok(msg) = rx.recv().await {
                println!("Dashboard received: {msg}");
            }
        });
    }
}

async fn dashboard_handler(
    Extension(state): Extension<Arc<AppState>>,
    Extension(user): Extension<User>,
) -> Html<String> {
    let mut ctx = tera::Context::new();
    ctx.insert("username", &user.id);
    let html = state.conduit
        .render("dashboard/dashboard", &ctx)
        .unwrap_or_else(|e| format!("Template error: {e}"));
    Html(html)
}
