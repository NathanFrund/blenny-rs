use axum::{response::Html, routing::get, Extension, Router};
use std::sync::Arc;
use crate::{blenny_module, BlennyModule, AppState, transport::ServerMessage};

#[derive(Default)]
#[blenny_module]
pub struct TestBroadcastModule;

impl BlennyModule for TestBroadcastModule {
    fn name(&self) -> &'static str { "TestBroadcast" }

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
) -> impl axum::response::IntoResponse {
    let category = params.get("category").cloned().unwrap_or_else(|| "ui".to_string());
    state.hub.broadcast(ServerMessage {
        category: category.clone(),
        html: Some(format!("<p>Broadcasted {category} at {}</p>", chrono::Local::now().format("%H:%M:%S"))),
        signals: None,
    });
    format!("Sent {category}")
}