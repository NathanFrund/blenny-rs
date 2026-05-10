use axum::{
    extract::{Extension, Query},
    response::sse::{Event, KeepAlive, Sse},
};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::sync::broadcast;
use tokio_stream::Stream;

use crate::app_state::AppState;

/// A message that can be sent to all connected SSE/WS clients.
#[derive(Clone, Debug)]
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
}

impl TransportHub {
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel::<ServerMessage>(256);
        TransportHub {
            tx,
            topics: Arc::new(RwLock::new(HashMap::new())),
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
}

/// SSE endpoint with optional intent filter.
/// If no ?intent= query parameter is given, all message categories are sent.
/// Example: /sse?intent=ui,notification
pub async fn sse_handler(
    Extension(state): Extension<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>> {
    // Determine if we should apply server‑side intent filtering.
    let do_server_filter = !state.encoder.filters_client_side();
    let intent_param = params.get("intent");
    let do_filter = intent_param.is_some();           // filter only if ?intent is present
    let intents: HashSet<String> = intent_param
        .map(|v| v.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();

    let mut rx = state.hub.subscribe();

    let stream = async_stream::stream! {
        loop {
            match rx.recv().await {
                Ok(msg) => {
                    println!("SSE handler got message: {:?}", msg);
                    // Apply server‑side filter only if the encoder doesn't handle it
                    // AND the client requested filtering via ?intent parameter.
                    if do_server_filter && do_filter && !intents.contains(&msg.category) {
                        continue;
                    }
                    let event = state.encoder.to_event(&msg);
                    yield Ok(event);
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    };
    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}
