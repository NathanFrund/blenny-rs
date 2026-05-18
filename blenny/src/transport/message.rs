// blenny/src/transport/message.rs

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ServerMessage {
    pub category: String,
    pub html: Option<String>,
    pub signals: Option<String>,
}
