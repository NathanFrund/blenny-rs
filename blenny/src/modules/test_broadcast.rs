use crate::{AppState, BlennyModule, blenny_module, transport::ServerMessage};
use axum::{
    Extension, Router,
    response::{Html, IntoResponse},
    routing::get,
};
use std::sync::Arc;

#[derive(Default)]
#[blenny_module]
pub struct TestBroadcastModule;

impl BlennyModule for TestBroadcastModule {
    fn name(&self) -> &'static str {
        "TestBroadcast"
    }

    fn register_routes(&self, router: Router) -> Router {
        router
            .route("/test-page", get(test_page))
            .route("/trigger-broadcast", get(trigger_broadcast))
    }
}

// Serve a simple HTML page that connects to /sse and shows messages.
async fn test_page() -> impl axum::response::IntoResponse {
    Html(include_str!("../../templates/test_broadcast.html"))
}

// Broadcast a message; category from query param, default "ui".
async fn trigger_broadcast(
    Extension(state): Extension<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let category = params
        .get("category")
        .cloned()
        .unwrap_or_else(|| "ui".to_string());
    let ts = chrono::Local::now().format("%H:%M:%S").to_string();

    let (html, signals) = match category.as_str() {
        "data" => (
            None,
            Some(format!(
                r#"{{"stock":"BLEN","price":{},"ts":"{}"}}"#,
                (42).to_string(),
                ts
            )),
        ),
        "command" => (Some(format!("console.log('command at {}')", ts)), None),
        _ => (
            Some(format!("<p>Broadcasted {} at {}</p>", category, ts)),
            None,
        ),
    };

    state.hub.broadcast(ServerMessage {
        category: category.clone(),
        html,
        signals,
    });
    format!("Sent {}", category)
}
