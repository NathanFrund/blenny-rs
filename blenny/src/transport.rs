use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::{Extension, Query},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    response::{
        Response,
        sse::{Event, KeepAlive, Sse},
    },
};
use futures::{SinkExt, StreamExt};
use std::collections::{HashMap, HashSet};
use std::convert::Infallible;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::app_state::AppState;

/// A message that can be sent to all connected SSE/WS clients.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ServerMessage {
    pub category: String,
    pub html: Option<String>,
    pub signals: Option<String>,
}

/// Configuration for the TransportHub's broadcast channels.
/// Allows tuning buffer sizes based on application requirements.
#[derive(Debug, Clone)]
pub struct TransportHubConfig {
    /// Buffer size for the global broadcast channel (default: 256).
    /// Slow subscribers will be dropped if they fall more than this many messages behind.
    pub global_broadcast_buffer: usize,

    /// Buffer size for topic-based pub/sub channels (default: 64).
    pub topic_buffer: usize,

    /// Buffer size for per-user direct message channels (default: 64).
    pub user_buffer: usize,
}

impl Default for TransportHubConfig {
    fn default() -> Self {
        Self {
            global_broadcast_buffer: 256,
            topic_buffer: 64,
            user_buffer: 64,
        }
    }
}

/// Handle to track a user connection.
/// When dropped, automatically unregisters the connection from the hub.
/// This prevents stale user channels when the same user connects multiple times (e.g., multiple tabs).
#[derive(Clone)]
pub struct ConnectionHandle {
    user_id: String,
    connection_id: Uuid,
    hub: Arc<TransportHub>,
}

impl Drop for ConnectionHandle {
    fn drop(&mut self) {
        self.hub
            .unregister_connection(&self.user_id, self.connection_id);
    }
}

/// Shared hub that holds the broadcast channels for real‑time messages.
///
/// The TransportHub manages three independent message flows:
///
/// 1. **Global Broadcast** – Sent to all connected clients via `broadcast_html()`, `broadcast_data()`, or `broadcast()`.
///    Used for public data, announcements, and shared state updates.
///
/// 2. **Topic-based Pub/Sub** – Per-topic broadcast channels for inter-module communication.
///    Modules can publish to topics (e.g., "order.created") and subscribe to them without direct coupling.
///
/// 3. **Per-User Direct Messaging** – Private channels for authenticated users.
///    Only the specified user receives these messages via `direct_html_to_user()` or `direct_data_to_user()`.
///
/// Connection IDs ensure that when a user connects multiple times (e.g., multiple browser tabs),
/// each connection gets its own receiver and closing one tab doesn't affect others.
#[derive(Clone)]
pub struct TransportHub {
    // Global client broadcast (SSE, WS)
    tx: broadcast::Sender<ServerMessage>,

    // Topic-based channels for inter-module messaging
    topics: Arc<RwLock<HashMap<String, broadcast::Sender<String>>>>,

    // Per-user channels for direct messaging, keyed by (user_id, connection_id)
    // Map: user_id -> (connection_id -> sender)
    users: Arc<RwLock<HashMap<String, HashMap<Uuid, broadcast::Sender<ServerMessage>>>>>,

    // Configuration for buffer sizes
    config: TransportHubConfig,
}

impl TransportHub {
    /// Create a new TransportHub with default buffer sizes.
    pub fn new() -> Self {
        Self::with_config(TransportHubConfig::default())
    }

    /// Create a new TransportHub with custom buffer sizes.
    pub fn with_config(config: TransportHubConfig) -> Self {
        let (tx, _rx) = broadcast::channel::<ServerMessage>(config.global_broadcast_buffer);
        TransportHub {
            tx,
            topics: Arc::new(RwLock::new(HashMap::new())),
            users: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    // ---------- global client broadcast ----------

    /// Subscribe to the global broadcast.
    /// Call from SSE/WS handlers to receive messages sent via `broadcast_html()`, etc.
    pub fn subscribe(&self) -> broadcast::Receiver<ServerMessage> {
        self.tx.subscribe()
    }

    /// Broadcast raw HTML to all connected clients.
    /// Messages are tagged with category "ui".
    pub fn broadcast_html(&self, html: &str) {
        let _ = self.tx.send(ServerMessage {
            category: "ui".into(),
            html: Some(html.into()),
            signals: None,
        });
    }

    /// Broadcast a raw data message to all connected clients.
    /// Messages are tagged with category "data".
    pub fn broadcast_data(&self, data: &str) {
        let _ = self.tx.send(ServerMessage {
            category: "data".into(),
            html: None,
            signals: Some(data.into()),
        });
    }

    /// Broadcast a generic ServerMessage to all connected clients.
    pub fn broadcast(&self, msg: ServerMessage) {
        let _ = self.tx.send(msg);
    }

    // ---------- topic‑based pub/sub ----------

    /// Publish a string message to a topic.
    /// Creates the topic channel if it doesn't already exist.
    /// Returns Ok if the message was sent, Err if all subscribers are gone.
    pub fn publish(&self, topic: &str, message: String) -> Result<(), BroadcastError> {
        let mut topics = self
            .topics
            .write()
            .map_err(|_| BroadcastError::HubPoisoned)?;

        let sender = topics.entry(topic.to_string()).or_insert_with(|| {
            let (tx, _rx) = broadcast::channel::<String>(self.config.topic_buffer);
            tx
        });

        sender
            .send(message)
            .map_err(|_| BroadcastError::NoReceivers)?;

        Ok(())
    }

    /// Subscribe to a topic. If the topic doesn't exist yet, create its channel.
    /// Returns a receiver that will receive future messages on that topic.
    pub fn subscribe_topic(
        &self,
        topic: &str,
    ) -> Result<broadcast::Receiver<String>, BroadcastError> {
        let mut topics = self
            .topics
            .write()
            .map_err(|_| BroadcastError::HubPoisoned)?;

        let sender = topics.entry(topic.to_string()).or_insert_with(|| {
            let (tx, _rx) = broadcast::channel::<String>(self.config.topic_buffer);
            tx
        });

        Ok(sender.subscribe())
    }

    // ---------- per-user direct messaging ----------

    /// Register a user connection and return a receiver for direct messages.
    /// A connection ID is automatically generated to track this specific connection,
    /// allowing the same user to have multiple simultaneous connections.
    ///
    /// The returned `ConnectionHandle` should be kept in scope for the lifetime of the connection.
    /// When dropped, it automatically unregisters the connection.
    pub fn register_user(
        &self,
        user_id: &str,
    ) -> Result<(broadcast::Receiver<ServerMessage>, ConnectionHandle), BroadcastError> {
        let (tx, rx) = broadcast::channel::<ServerMessage>(self.config.user_buffer);
        let connection_id = Uuid::new_v4();

        let mut users = self
            .users
            .write()
            .map_err(|_| BroadcastError::HubPoisoned)?;

        // Get or create the inner map for this user
        let user_map = users.entry(user_id.to_string()).or_default();
        user_map.insert(connection_id, tx);

        let handle = ConnectionHandle {
            user_id: user_id.to_string(),
            connection_id,
            hub: Arc::new(self.clone()),
        };

        Ok((rx, handle))
    }

    /// Unregister a specific user connection by ID.
    /// This is called automatically when a ConnectionHandle is dropped.
    fn unregister_connection(&self, user_id: &str, connection_id: Uuid) {
        if let Ok(mut users) = self.users.write() {
            if let Some(connections) = users.get_mut(user_id) {
            connections.remove(&connection_id);
            if connections.is_empty() {
                users.remove(user_id);
            }
        }
            tracing::debug!(
                "Unregistered connection {} for user {}",
                connection_id,
                user_id
            );
        } else {
            tracing::warn!("Failed to unregister connection: hub poisoned");
        }
    }

    /// Send a message directly to all connections of a specific user.
    pub fn direct_to_user(&self, user_id: &str, msg: ServerMessage) {
        if let Some(user_map) = self.users.read().ok() {
            if let Some(connections) = user_map.get(user_id) {
                for tx in connections.values() {
                    if tx.send(msg.clone()).is_ok() {
                        // Optionally track sent messages
                    }
                }
            } else {
                tracing::debug!("No active connections for user {}", user_id);
            }
        } else {
            tracing::warn!("Failed to read user map for direct message");
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

impl Default for TransportHub {
    fn default() -> Self {
        Self::new()
    }
}

/// Error type for TransportHub operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BroadcastError {
    /// All subscribers to the channel have been dropped.
    NoReceivers,

    /// The RwLock protecting the hub's state is poisoned (a thread panicked while holding it).
    HubPoisoned,
}

impl std::fmt::Display for BroadcastError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoReceivers => write!(f, "No receivers for this message"),
            Self::HubPoisoned => write!(f, "Hub state is poisoned"),
        }
    }
}

impl std::error::Error for BroadcastError {}

// ---------- SSE Handler ----------

/// SSE endpoint with optional intent filter.
///
/// Requires authentication if `config.transport_auth_required` is true.
/// If no `?intent=` query parameter is given, all message categories are sent.
///
/// Example: `/sse?intent=ui,notification`
///
/// Authenticated users receive both global broadcasts and personal direct messages.
/// Unauthenticated connections (if allowed) receive only global broadcasts.
pub async fn sse_handler(
    Extension(state): Extension<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    // Try to authenticate user
    let user = crate::auth::User::from_headers(&headers, &state.jwt_secret);

    // Enforce authentication if required
    if state.config.transport_auth_required && user.is_none() {
        return (StatusCode::UNAUTHORIZED, "Authentication required").into_response();
    }

    // Determine if the encoder filters client-side
    let do_server_filter = !state.encoder.filters_client_side();

    // Parse intent query parameter if provided
    let intent_param = params.get("intent").cloned();
    let do_filter = intent_param.is_some();
    let intents: HashSet<String> = intent_param
        .map(|v| v.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();

    Sse::new(sse_stream(
        state,
        user,
        do_server_filter,
        do_filter,
        intents,
    ))
    .keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
    .into_response()
}

/// Stream generator for SSE.
/// Handles subscription to both global and personal channels, applies intent filtering,
/// and yields events for the SSE response.
///
/// This is extracted as a separate function to:
/// 1. Make the type explicit (`impl Stream<Item = Result<Event, Infallible>>`)
/// 2. Reduce cognitive load in the handler
/// 3. Allow reuse by other real-time endpoints
/// 4. Improve testability
fn sse_stream(
    state: Arc<AppState>,
    user: Option<crate::auth::User>,
    do_server_filter: bool,
    do_filter: bool,
    intents: HashSet<String>,
) -> impl futures::Stream<Item = Result<Event, Infallible>> {
    async_stream::stream! {
        // Subscribe to global broadcast
        let mut global_rx = state.hub.subscribe();

        // Register personal channel – keep handle alive for the stream duration
        let (mut personal_rx, _connection_guard) = if let Some(ref user) = user {
            match state.hub.register_user(&user.id) {
                Ok((rx, handle)) => (Some(rx), Some(handle)),
                Err(e) => {
                    tracing::error!("Failed to register user for SSE: {}", e);
                    yield Ok(Event::default().comment("failed to register for personal messages"));
                    (None, None)
                }
            }
        } else {
            (None, None)
        };

        // _connection_guard lives until the stream ends, then unregisters

        loop {
            let msg = if let Some(ref mut personal) = personal_rx {
                tokio::select! {
                    Ok(msg) = global_rx.recv() => msg,
                    Ok(msg) = personal.recv() => msg,
                    else => break,
                }
            } else {
                match global_rx.recv().await {
                    Ok(msg) => msg,
                    Err(_) => break,
                }
            };

            // Apply server-side intent filtering if requested
            if do_server_filter && do_filter && !intents.contains(msg.category.as_str()) {
                continue;
            }

            // Convert ServerMessage to SSE Event
            let event = state.encoder.to_event(&msg);
            yield Ok(event);
        }
    }
}

// ---------- WebSocket Handler ----------

/// WebSocket endpoint with optional intent filter.
///
/// Requires authentication if `config.transport_auth_required` is true.
/// If no `?intent=` query parameter is given, all message categories are sent.
///
/// Example: `/ws?intent=ui,notification`
///
/// Authenticated users receive both global broadcasts and personal direct messages.
/// Unauthenticated connections (if allowed) receive only global broadcasts.
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

/// Handle a WebSocket connection.
/// Manages bidirectional communication: forwards hub messages to client,
/// discards or logs client messages.
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

    // Register and subscribe to personal channel if authenticated
    let (mut personal_rx, _ws_guard) = if let Some(ref user) = user {
        match state.hub.register_user(&user.id) {
            Ok((rx, handle)) => (Some(rx), Some(handle)),
            Err(e) => {
                tracing::error!("Failed to register user for WebSocket: {}", e);
                (None, None)
            }
        }
    } else {
        (None, None)
    };

    // Task to forward messages from hub to WebSocket
    let send_task = async move {
        loop {
            let msg = if let Some(ref mut personal) = personal_rx {
                tokio::select! {
                    Ok(msg) = global_rx.recv() => msg,
                    Ok(msg) = personal.recv() => msg,
                    else => break,
                }
            } else {
                match global_rx.recv().await {
                    Ok(msg) => msg,
                    Err(_) => break,
                }
            };

            // Apply intent filter
            if do_filter && !intents.contains(msg.category.as_str()) {
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
            if let Err(e) = sender.send(Message::Text(payload.into())).await {
                tracing::debug!("Failed to send WebSocket message: {}", e);
                break;
            }
        }
    };

    // Receive task – just discard incoming messages (or log them for future use)
    let recv_task = async {
        while let Some(Ok(_msg)) = futures::StreamExt::next(&mut receiver).await {
            // Optionally log or handle client->server messages here.
            // For now, we discard them (bidirectional isn't needed for hub broadcasts).
        }
    };

    // Run both tasks concurrently; whichever finishes first breaks the connection
    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }

    tracing::debug!(
        "WebSocket connection closed for user: {:?}",
        user.as_ref().map(|u| &u.id)
    );
}
