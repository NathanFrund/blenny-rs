// blenny/src/transport/hub.rs
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tokio::sync::broadcast;
use uuid::Uuid;

use super::message::ServerMessage;

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
