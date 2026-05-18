// blenny/src/transport/mod.rs
pub mod message;
pub mod hub;
pub mod sse;
pub mod ws;

pub use message::ServerMessage;
pub use hub::{TransportHubConfig, ConnectionHandle, TransportHub, BroadcastError};
pub use sse::sse_handler;
pub use ws::ws_handler;

/// Select the next message from either the global or personal receiver.
/// Returns `None` if both channels are closed.
pub(crate) async fn select_message(
    global: &mut tokio::sync::broadcast::Receiver<ServerMessage>,
    personal: &mut Option<tokio::sync::broadcast::Receiver<ServerMessage>>,
) -> Option<ServerMessage> {
    loop {
        if let Some(prx) = personal {
            tokio::select! {
                res = global.recv() => {
                    match res {
                        Ok(msg) => return Some(msg),
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => return None,
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                            tracing::warn!("Global receiver lagged by {} messages", skipped);
                            continue;
                        }
                    }
                }
                res = prx.recv() => {
                    match res {
                        Ok(msg) => return Some(msg),
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => return None,
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                            tracing::warn!("Personal receiver lagged by {} messages", skipped);
                            continue;
                        }
                    }
                }
            }
        } else {
            match global.recv().await {
                Ok(msg) => return Some(msg),
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return None,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                    tracing::warn!("Global receiver lagged by {} messages", skipped);
                    continue;
                }
            }
        }
    }
}
