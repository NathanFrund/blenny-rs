// blenny/src/transport/mod.rs
pub mod hub;
pub mod message;
pub mod sse;
pub mod ws;

pub use hub::{BroadcastError, ConnectionHandle, TransportHub, TransportHubConfig};
pub use message::ServerMessage;
pub use sse::sse_handler;
pub use ws::ws_handler;

use std::collections::{HashMap, HashSet};

/// Parse the `?intent=` query parameter into a set of allowed categories.
/// Returns `(do_filter, intents)` where `do_filter` is true if the client
/// supplied an intent parameter, and `intents` is the set of categories to allow.
pub(crate) fn parse_intents(params: &HashMap<String, String>) -> (bool, HashSet<String>) {
    let intent_param = params.get("intent");
    let do_filter = intent_param.is_some();
    let intents: HashSet<String> = intent_param
        .map(|v| v.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();
    (do_filter, intents)
}

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
