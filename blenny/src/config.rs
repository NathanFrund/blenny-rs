// blenny/src/config.rs
use figment::{
    Figment,
    providers::{Env, Format, Json},
};
use serde::Deserialize;

/// Configuration loaded from multiple sources.
/// Priority (highest to lowest):
///   1. BLENNY_* environment variables
///   2. blenny.json file
///   3. Rust defaults (below)
#[derive(Debug, Clone, Deserialize)]
pub struct BlennyConfig {
    /// Server port (default: 8081)
    #[serde(default = "default_port")]
    pub port: u16,

    /// Directory for hot‑reloading templates; `None` means use embedded.
    #[serde(default)]
    pub template_dir: Option<String>,

    /// Secret key for JWT signing / verification.
    #[serde(default = "default_jwt_secret")]
    pub jwt_secret: String,

    /// SSE encoder: "standard" or "datastar" (future).
    #[serde(default = "default_encoder")]
    pub encoder: String,

    /// Enable WebSocket endpoint (/ws) alongside SSE.
    #[serde(default = "default_websocket")]
    pub websocket: bool,

    /// Require authentication for SSE/WS transports (default: true).
    #[serde(default = "default_transport_auth_required")]
    pub transport_auth_required: bool,
}

fn default_port() -> u16 { 8081 }
fn default_jwt_secret() -> String { "dev-secret".into() }
fn default_encoder() -> String { "standard".into() }
fn default_websocket() -> bool { false }
fn default_transport_auth_required() -> bool { true }

impl Default for BlennyConfig {
    fn default() -> Self {
        BlennyConfig {
            port: default_port(),
            template_dir: None,
            jwt_secret: default_jwt_secret(),
            encoder: default_encoder(),
            websocket: default_websocket(),
            transport_auth_required: default_transport_auth_required(),
        }
    }
}

impl BlennyConfig {
    /// Load configuration from environment and a JSON file, falling back to defaults.
    pub fn load() -> Self {
        Figment::new()
            .merge(Env::prefixed("BLENNY_"))
            .merge(Json::file("blenny.json"))
            .extract()
            .unwrap_or_else(|err| {
                eprintln!("Invalid config, using defaults: {err}");
                Self::default()
            })
    }
}