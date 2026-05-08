use axum::{
    extract::Extension,
    response::sse::{Event, KeepAlive, Sse},
};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio_stream::Stream;

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
    tx: broadcast::Sender<ServerMessage>,
}

impl TransportHub {
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel::<ServerMessage>(256);
        TransportHub { tx }
    }

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
}

/// SSE endpoint handler.
pub async fn sse_handler(
    Extension(hub): Extension<Arc<TransportHub>>,
) -> Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>> {
    let mut rx = hub.subscribe();
    let stream = async_stream::stream! {
        loop {
            match rx.recv().await {
                Ok(msg) => {
                    // Determine payload: prefer html, fallback to signals, skip if none.
                    let data = if let Some(html) = msg.html {
                        html
                    } else if let Some(signals) = msg.signals {
                        signals
                    } else {
                        continue;
                    };
                    yield Ok(Event::default().data(data));
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
