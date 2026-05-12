use axum::{
    extract::{Extension, Query},
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    http::{HeaderMap, StatusCode},
    response::{Response, sse::{Event, KeepAlive, Sse}},
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::sync::broadcast;
use tokio_stream::{wrappers::BroadcastStream, StreamExt as TokioStreamExt};

use crate::app_state::AppState;

/// A message that can be sent to all connected SSE/WS clients.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ServerMessage {
    pub category: String,
    pub html: Option<String>,
    pub signals: Option<String>,
}

/// Shared hub that holds the broadcast channel for real‑time messages.
#[derive(Clone)]
pub struct TransportHub {
    // Global client broadcast (SSE, future WS)
    tx: broadcast::Sender<ServerMessage>,
    // Topic‑based channels for inter‑module messaging
    topics: Arc<RwLock<HashMap<String, broadcast::Sender<String>>>>,
    // Per-user channels for direct messaging
    users: Arc<RwLock<HashMap<String, broadcast::Sender<ServerMessage>>>>,
}

impl Default for TransportHub {
    fn default() -> Self {
        Self::new()
    }
}

impl TransportHub {
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel::<ServerMessage>(256);
        TransportHub {
            tx,
            topics: Arc::new(RwLock::new(HashMap::new())),
            users: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    // ---------- global client broadcast ----------

    /// Subscribe to the broadcast (call from SSE/WS handlers).
    pub fn subscribe(&self) -> broadcast::Receiver<ServerMessage> {
        self.tx.subscribe()
    }

    /// Broadcast raw HTML to all connected clients.
    pub fn broadcast_html(&self, html: &str) {
        let _ = self.tx.send(ServerMessage {
            category: "ui".into(),
            html: Some(html.into()),
            signals: None,
        });
    }

    /// Broadcast a raw data message.
    pub fn broadcast_data(&self, data: &str) {
        let _ = self.tx.send(ServerMessage {
            category: "ui".into(),
            html: None,
            signals: Some(data.into()),
        });
    }

    /// Broadcast a generic ServerMessage.
    pub fn broadcast(&self, msg: ServerMessage) {
        let _ = self.tx.send(msg);
    }

    // ---------- topic‑based pub/sub ----------

    /// Publish a string message to a topic. Creates the topic if it doesn't exist.
    pub fn publish(&self, topic: &str, message: String) {
        let mut topics = self.topics.write().unwrap();
        let sender = topics.entry(topic.to_string()).or_insert_with(|| {
            let (tx, _rx) = broadcast::channel::<String>(64);
            tx
        });
        let _ = sender.send(message);
    }

    /// Subscribe to a topic. If the topic doesn't exist yet, create its channel.
    /// Returns a receiver that will receive future messages.
    pub fn subscribe_topic(&self, topic: &str) -> broadcast::Receiver<String> {
        let mut topics = self.topics.write().unwrap();
        let sender = topics.entry(topic.to_string()).or_insert_with(|| {
            let (tx, _rx) = broadcast::channel::<String>(64);
            tx
        });
        sender.subscribe()
    }

    /// Register a user (or replace existing) and return a personal receiver for direct messages.
    pub fn register_user(&self, user_id: &str) -> broadcast::Receiver<ServerMessage> {
        let (tx, rx) = broadcast::channel::<ServerMessage>(64);
        self.users.write().unwrap().insert(user_id.to_string(), tx);
        rx
    }

    /// Remove a user's personal sender, stopping further direct messages.
    pub fn unregister_user(&self, user_id: &str) {
        self.users.write().unwrap().remove(user_id);
    }

    /// Send a message directly to a specific user.
    pub fn direct_to_user(&self, user_id: &str, msg: ServerMessage) {
        if let Some(tx) = self.users.read().unwrap().get(user_id) {
            let _ = tx.send(msg);
        }
    }

    /// Convenience: send HTML directly to a user.
    pub fn direct_html_to_user(&self, user_id: &str, html: &str) {
        self.direct_to_user(
            user_id,
            ServerMessage {
                category: "ui".into(),
                html: Some(html.into()),
                signals: None,
            },
        );
    }

    /// Convenience: send data directly to a user.
    pub fn direct_data_to_user(&self, user_id: &str, data: &str) {
        self.direct_to_user(
            user_id,
            ServerMessage {
                category: "data".into(),
                html: None,
                signals: Some(data.into()),
            },
        );
    }
}

/// SSE endpoint with optional intent filter.
/// If no ?intent= query parameter is given, all message categories are sent.
/// Example: /sse?intent=ui,notification
pub async fn sse_handler(
    Extension(state): Extension<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    // Try to authenticate user
    let user = crate::auth::User::from_headers(&headers, &state.jwt_secret);

    if state.config.transport_auth_required && user.is_none() {
        return (StatusCode::UNAUTHORIZED, "Authentication required").into_response();
    }

    // Determine if we should apply server‑side intent filtering.
    let do_server_filter = !state.encoder.filters_client_side();
    let intent_param = params.get("intent");
    let do_filter = intent_param.is_some();           // filter only if ?intent is present
    let intents: HashSet<String> = intent_param
        .map(|v| v.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();

    let global_rx = state.hub.subscribe();

    let stream = TokioStreamExt::filter_map(BroadcastStream::new(global_rx), move |result| match result {
        Ok(msg) => {
            if do_server_filter && do_filter && intents.contains(&msg.category) {
                None
            } else {
                Some(Ok::<Event, std::convert::Infallible>(state.encoder.to_event(&msg)))
            }
        }
        Err(_) => None,
    });



    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    ).into_response()
}

/// WebSocket endpoint with optional intent filter.
/// If no ?intent= query parameter is given, all message categories are sent.
/// Authenticated users (via JWT cookie/header) also receive personal messages.
/// Sends raw HTML or data payloads – not JSON‑wrapped.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Extension(state): Extension<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    // Parse intents (same as SSE)
    let do_server_filter = !state.encoder.filters_client_side();
    let intents: HashSet<String> = if do_server_filter {
        params
            .get("intent")
            .map(|v| v.split(',').map(|s| s.trim().to_string()).collect())
            .unwrap_or_default()
    } else {
        HashSet::new()
    };
    let do_filter = params.contains_key("intent");

    // Authenticate user
    let user = crate::auth::User::from_headers(&headers, &state.jwt_secret);

    // Require authentication if configured
    if state.config.transport_auth_required && user.is_none() {
        return (StatusCode::UNAUTHORIZED, "Authentication required").into_response();
    }

    ws.on_upgrade(move |socket| handle_ws(socket, state, intents, do_filter, user))
}

async fn handle_ws(
    socket: WebSocket,
    state: Arc<AppState>,
    intents: HashSet<String>,
    do_filter: bool,
    user: Option<crate::auth::User>,
) {
    let (mut sender, mut receiver) = socket.split();

    // Subscribe to global broadcast
    let mut global_rx = state.hub.subscribe();

    // Register personal channel if authenticated
    let mut personal_rx = user
        .as_ref()
        .map(|u| state.hub.register_user(&u.id));

    // Task to forward messages from hub to WebSocket
    let send_task = async move {
        loop {
            let msg = if let Some(ref mut personal) = personal_rx {
                tokio::select! {
                    global_msg = global_rx.recv() => {
                        match global_msg {
                            Ok(msg) => msg,
                            Err(_) => break,
                        }
                    }
                    personal_msg = personal.recv() => {
                        match personal_msg {
                            Ok(msg) => msg,
                            Err(_) => break,
                        }
                    }
                }
            } else {
                match global_rx.recv().await {
                    Ok(msg) => msg,
                    Err(_) => break,
                }
            };

            // Apply intent filter
            if do_filter && !intents.contains(&msg.category) {
                continue;
            }

            // Extract raw payload – html takes precedence, then signals
            let payload = if let Some(html) = &msg.html {
                html.clone()
            } else if let Some(signals) = &msg.signals {
                signals.clone()
            } else {
                continue; // nothing to send
            };

            // Send as a text frame
            if sender.send(Message::Text(payload.into())).await.is_err() {
                break;
            }
        }
    };

    // Receive task – just discard incoming messages (or log them)
    let recv_task = async {
        while let Some(Ok(_msg)) = futures::StreamExt::next(&mut receiver).await {
            // optionally log or handle client->server messages here
        }
    };

    // Run both tasks concurrently; whichever finishes first breaks the connection
    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }

    // Clean up personal sender if registered
    if let Some(user) = user {
        state.hub.unregister_user(&user.id);
    }
}
