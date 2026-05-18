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

// ====================== TYPES ======================

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ServerMessage {
    pub category: String,
    pub html: Option<String>,
    pub signals: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TransportHubConfig {
    pub global_broadcast_buffer: usize,
    pub topic_buffer: usize,
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

#[derive(Clone)]
pub struct ConnectionHandle {
    user_id: String,
    connection_id: Uuid,
    users: Arc<RwLock<HashMap<String, HashMap<Uuid, broadcast::Sender<ServerMessage>>>>>,
}

impl Drop for ConnectionHandle {
    fn drop(&mut self) {
        if let Ok(mut users) = self.users.write() {
            if let Some(conns) = users.get_mut(&self.user_id) {
                conns.remove(&self.connection_id);
                if conns.is_empty() {
                    users.remove(&self.user_id);
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct TransportHub {
    tx: broadcast::Sender<ServerMessage>,
    topics: Arc<RwLock<HashMap<String, broadcast::Sender<String>>>>,
    users: Arc<RwLock<HashMap<String, HashMap<Uuid, broadcast::Sender<ServerMessage>>>>>,
    config: TransportHubConfig,
}

impl TransportHub {
    pub fn new() -> Self {
        Self::with_config(TransportHubConfig::default())
    }

    pub fn with_config(config: TransportHubConfig) -> Self {
        let (tx, _rx) = broadcast::channel(config.global_broadcast_buffer);
        Self {
            tx,
            topics: Arc::new(RwLock::new(HashMap::new())),
            users: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    // ---------- Global ----------
    pub fn subscribe(&self) -> broadcast::Receiver<ServerMessage> {
        self.tx.subscribe()
    }

    pub fn broadcast_html(&self, html: &str) {
        let _ = self.tx.send(ServerMessage {
            category: "ui".into(),
            html: Some(html.into()),
            signals: None,
        });
    }

    pub fn broadcast_data(&self, data: &str) {
        let _ = self.tx.send(ServerMessage {
            category: "data".into(),
            html: None,
            signals: Some(data.into()),
        });
    }

    pub fn broadcast(&self, msg: ServerMessage) {
        let _ = self.tx.send(msg);
    }

    // ---------- Topics ----------
    pub fn publish(&self, topic: &str, message: String) -> Result<(), BroadcastError> {
        let mut topics = self
            .topics
            .write()
            .map_err(|_| BroadcastError::HubPoisoned)?;
        let sender = topics.entry(topic.to_string()).or_insert_with(|| {
            let (tx, _rx) = broadcast::channel(self.config.topic_buffer);
            tx
        });
        sender
            .send(message)
            .map_err(|_| BroadcastError::NoReceivers)?;
        Ok(())
    }

    pub fn subscribe_topic(
        &self,
        topic: &str,
    ) -> Result<broadcast::Receiver<String>, BroadcastError> {
        let mut topics = self
            .topics
            .write()
            .map_err(|_| BroadcastError::HubPoisoned)?;
        let sender = topics.entry(topic.to_string()).or_insert_with(|| {
            let (tx, _rx) = broadcast::channel(self.config.topic_buffer);
            tx
        });
        Ok(sender.subscribe())
    }

    // ---------- Per-User ----------
    pub fn register_user(
        &self,
        user_id: &str,
    ) -> Result<(broadcast::Receiver<ServerMessage>, ConnectionHandle), BroadcastError> {
        let (tx, rx) = broadcast::channel(self.config.user_buffer);
        let connection_id = Uuid::new_v4();

        let mut users = self
            .users
            .write()
            .map_err(|_| BroadcastError::HubPoisoned)?;
        users
            .entry(user_id.to_string())
            .or_default()
            .insert(connection_id, tx);

        let handle = ConnectionHandle {
            user_id: user_id.to_string(),
            connection_id,
            users: Arc::clone(&self.users),
        };

        Ok((rx, handle))
    }

    pub fn direct_to_user(&self, user_id: &str, msg: ServerMessage) {
        if let Ok(users) = self.users.read() {
            if let Some(conns) = users.get(user_id) {
                for tx in conns.values() {
                    let _ = tx.send(msg.clone());
                }
            }
        }
    }

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BroadcastError {
    NoReceivers,
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

// ====================== SSE ======================

pub async fn sse_handler(
    Extension(state): Extension<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    let mut user = crate::auth::User::from_headers(&headers, &state.jwt_secret);

    if user.is_none() {
        if let Some(token) = params.get("token").or(params.get("blenny_token")) {
            user = crate::auth::User::from_token(token, &state.jwt_secret);
        }
    }

    if state.config.transport_auth_required && user.is_none() {
        return (StatusCode::UNAUTHORIZED, "Authentication required").into_response();
    }

    let do_server_filter = !state.encoder.filters_client_side();
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

fn sse_stream(
    state: Arc<AppState>,
    user: Option<crate::auth::User>,
    do_server_filter: bool,
    do_filter: bool,
    intents: HashSet<String>,
) -> impl futures::Stream<Item = Result<Event, Infallible>> {
    async_stream::stream! {
        let mut global_rx = state.hub.subscribe();

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

        loop {
            let msg = match select_message(&mut global_rx, &mut personal_rx).await {
                Some(msg) => msg,
                None => break,
            };

            if do_server_filter && do_filter && !intents.contains(msg.category.as_str()) {
                continue;
            }

            let event = state.encoder.to_event(&msg);
            yield Ok(event);
        }
    }
}

// ====================== WebSocket ======================

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Extension(state): Extension<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> impl IntoResponse {
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

    let mut user = crate::auth::User::from_headers(&headers, &state.jwt_secret);

    if user.is_none() {
        if let Some(token) = params.get("token").or(params.get("blenny_token")) {
            user = crate::auth::User::from_token(token, &state.jwt_secret);
        }
    }

    if state.config.transport_auth_required && user.is_none() {
        return (StatusCode::UNAUTHORIZED, "Authentication required").into_response();
    }

    ws.on_failed_upgrade(|error| {
        tracing::error!("WebSocket upgrade failed: {}", error);
    })
    .on_upgrade(move |socket| handle_ws(socket, state, intents, do_filter, user))
}

async fn handle_ws(
    socket: WebSocket,
    state: Arc<AppState>,
    intents: HashSet<String>,
    do_filter: bool,
    user: Option<crate::auth::User>,
) {
    let (mut sender, mut receiver) = socket.split();
    let user_id_for_log = user.as_ref().map(|u| u.id.clone());

    let send_task = async move {
        let mut global_rx = state.hub.subscribe();

        // Register user and hold the guard inside the task
        let (mut personal_rx, _ws_guard) = if let Some(ref user) = user {
            match state.hub.register_user(&user.id) {
                Ok((rx, handle)) => (Some(rx), Some(handle)),
                Err(e) => {
                    tracing::error!("Failed to register user for WS: {}", e);
                    (None, None)
                }
            }
        } else {
            (None, None)
        };

        loop {
            let msg = match select_message(&mut global_rx, &mut personal_rx).await {
                Some(msg) => msg,
                None => break,
            };

            if do_filter && !intents.contains(msg.category.as_str()) {
                continue;
            }

            let payload = msg.html.or(msg.signals).unwrap_or_default();
            if sender.send(Message::Text(payload.into())).await.is_err() {
                break;
            }
        }
    };

    let recv_task = async { while let Some(Ok(_)) = receiver.next().await {} };

    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }

    tracing::debug!("WebSocket disconnected for user: {:?}", user_id_for_log);
}

// ====================== Helper ======================

/// Select the next message from either the global or personal receiver.
/// Returns `None` if both channels are closed.
async fn select_message(
    global: &mut broadcast::Receiver<ServerMessage>,
    personal: &mut Option<broadcast::Receiver<ServerMessage>>,
) -> Option<ServerMessage> {
    loop {
        if let Some(prx) = personal {
            tokio::select! {
                res = global.recv() => {
                    match res {
                        Ok(msg) => return Some(msg),
                        Err(broadcast::error::RecvError::Closed) => return None,
                        Err(broadcast::error::RecvError::Lagged(skipped)) => {
                            tracing::warn!("Global receiver lagged by {} messages", skipped);
                            continue;
                        }
                    }
                }
                res = prx.recv() => {
                    match res {
                        Ok(msg) => return Some(msg),
                        Err(broadcast::error::RecvError::Closed) => return None,
                        Err(broadcast::error::RecvError::Lagged(skipped)) => {
                            tracing::warn!("Personal receiver lagged by {} messages", skipped);
                            continue;
                        }
                    }
                }
            }
        } else {
            match global.recv().await {
                Ok(msg) => return Some(msg),
                Err(broadcast::error::RecvError::Closed) => return None,
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    tracing::warn!("Global receiver lagged by {} messages", skipped);
                    continue;
                }
            }
        }
    }
}
