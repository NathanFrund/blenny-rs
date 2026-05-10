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

#[cfg(feature = "datastar-sse")]
#[derive(Clone)]
pub struct DatastarEncoder;

#[cfg(feature = "datastar-sse")]
impl TransportEncoder for DatastarEncoder {
    /// Return only the raw payload bytes – no SSE framing.
    fn encode(&self, msg: &ServerMessage) -> Vec<u8> {
        match msg.category.as_str() {
            "ui" => msg.html.clone().unwrap_or_default().into_bytes(),
            "data" => msg.signals.clone().unwrap_or_default().into_bytes(),
            "command" => msg.html.clone().unwrap_or_default().into_bytes(),
            _ => msg
                .html
                .clone()
                .or_else(|| msg.signals.clone())
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

    /// Build a named SSE Event with the correct event type and data.
    fn to_event(&self, msg: &ServerMessage) -> Event {
        let encoded_bytes = self.encode(msg);
        let payload = String::from_utf8_lossy(&encoded_bytes);
        let event_type = match msg.category.as_str() {
            "ui" => "datastar-patch-elements",
            "data" => "datastar-patch-signals",
            "command" => "datastar-execute-script",
            _ => "blenny-notification",
        };
        Event::default().event(event_type).data(payload)
    }
}
