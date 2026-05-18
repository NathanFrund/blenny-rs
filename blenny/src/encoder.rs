// blenny/src/encoder.rs
use crate::transport::ServerMessage;
use axum::response::sse::Event;

/// Trait for encoding server‑sent event payloads.
/// Implementors decide the wire format and whether server‑side
/// intent filtering should be bypassed (because the client handles it).
pub trait TransportEncoder: Send + Sync {
    /// Convert a `ServerMessage` into raw bytes for the SSE stream.
    fn encode(&self, msg: &ServerMessage) -> Vec<u8>;

    /// The Content-Type header for the SSE stream.
    fn content_type(&self) -> &'static str;

    /// If true, the SSE handler skips the `?intent=` filter
    /// because the client performs its own filtering natively.
    fn filters_client_side(&self) -> bool {
        false
    }

    /// Build an SSE Event from the message. Default implementation uses
    /// the output of `encode()` as the `data:` field.
    fn to_event(&self, msg: &ServerMessage) -> Event {
        let bytes = self.encode(msg);
        let data = String::from_utf8_lossy(&bytes);
        Event::default().data(data)
    }
}

/// Standard Blenny JSON encoder.
#[derive(Clone)]
pub struct StandardEncoder;

impl TransportEncoder for StandardEncoder {
    fn encode(&self, msg: &ServerMessage) -> Vec<u8> {
        let payload = if let Some(html) = &msg.html {
            html.clone()
        } else if let Some(signals) = &msg.signals {
            signals.clone()
        } else {
            String::new()
        };
        // Simply send the payload as SSE data (the original behaviour)
        payload.into_bytes()
    }

    fn content_type(&self) -> &'static str {
        "text/event-stream"
    }

    // filters_client_side() defaults to false → server-side filtering
    // to_event() uses default implementation
}

// Official Datastar SDK based encoder
#[cfg(feature = "datastar-sse")]
#[derive(Clone)]
pub struct DatastarEncoder;

#[cfg(feature = "datastar-sse")]
impl TransportEncoder for DatastarEncoder {
    /// Return only the raw payload bytes – no SSE framing. (Used for testing)
    fn encode(&self, msg: &ServerMessage) -> Vec<u8> {
        match msg.category.as_str() {
            "ui" => msg.html.clone().unwrap_or_default().into_bytes(),
            "data" => msg.signals.clone().unwrap_or_default().into_bytes(),
            "command" => msg.html.clone().unwrap_or_default().into_bytes(),
            _ => msg
                .html
                .clone()
                .or(msg.signals.clone())
                .unwrap_or_default()
                .into_bytes(),
        }
    }

    fn content_type(&self) -> &'static str {
        "text/event-stream"
    }

    fn filters_client_side(&self) -> bool {
        true
    }

    /// Build the correct named SSE Event using the official Datastar SDK.
    fn to_event(&self, msg: &ServerMessage) -> Event {
        // Import datastar prelude to bring in the event types
        use datastar::prelude::*;

        match msg.category.as_str() {
            "ui" => {
                let html = msg.html.clone().unwrap_or_default();
                PatchElements::new(html)
                    .into_datastar_event()
                    .write_as_axum_sse_event()
            }
            "data" => {
                let signals = msg.signals.clone().unwrap_or_default();
                PatchSignals::new(signals)
                    .into_datastar_event()
                    .write_as_axum_sse_event()
            }
            "command" => {
                let script = msg.html.clone().unwrap_or_default();
                ExecuteScript::new(script)
                    .into_datastar_event()
                    .write_as_axum_sse_event()
            }
            // Fallback: send as a generic patch‑elements event.
            _ => {
                let data = msg.html.clone().or(msg.signals.clone()).unwrap_or_default();
                PatchElements::new(data)
                    .into_datastar_event()
                    .write_as_axum_sse_event()
            }
        }
    }
}
