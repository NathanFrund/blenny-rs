use crate::{AppState, BlennyModule, blenny_module, transport::ServerMessage, auth::User};
use axum::{
    Extension, Router,
    response::{Html, IntoResponse},
    routing::get,
};
use std::collections::HashSet;
use std::sync::Arc;

#[derive(Default)]
#[blenny_module]
pub struct TestBroadcastModule;

impl BlennyModule for TestBroadcastModule {
    fn name(&self) -> &'static str {
        "TestBroadcast"
    }

    fn public_routes(&self) -> HashSet<String> {
        let mut routes = HashSet::new();
        routes.insert("/test-page".to_string());
        routes.insert("/trigger-broadcast".to_string());
        routes
    }

    fn register_routes(&self, router: Router) -> Router {
        router
            .route("/test-page", get(test_page))
            .route("/trigger-broadcast", get(trigger_broadcast))
            .route("/panic", get(panic_test))
            .route("/direct-to-me", get(direct_to_me_handler))
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
                (42),
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

// Test route that panics to verify anti-fragile middleware
async fn panic_test() -> &'static str {
    panic!("Test panic - this should be caught by the anti-fragile middleware");
}

// Test route that sends a direct message to the logged-in user
async fn direct_to_me_handler(
    Extension(state): Extension<Arc<AppState>>,
    Extension(user): Extension<User>,
) -> impl IntoResponse {
    state.hub.direct_html_to_user(&user.id, "<p>Direct message!</p>");
    "Sent direct message"
}
