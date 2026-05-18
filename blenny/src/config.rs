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
    /// Priority (highest to lowest):
    ///   1. BLENNY_JWT_SECRET_FILE environment variable (path to file containing the secret)
    ///   2. BLENNY_JWT_SECRET environment variable / JSON `jwt_secret` configuration
    ///   3. Rust default ("dev-secret")
    #[serde(default = "default_jwt_secret")]
    pub jwt_secret: String,

    /// Path to a file containing the JWT secret (useful in production like Docker/K8s).
    /// If provided, the contents of this file takes precedence over `jwt_secret`.
    #[serde(default)]
    pub jwt_secret_file: Option<String>,

    /// SSE encoder: "standard" or "datastar" (future).
    #[serde(default = "default_encoder")]
    pub encoder: String,

    /// Enable WebSocket endpoint (/ws) alongside SSE.
    #[serde(default = "default_websocket")]
    pub websocket: bool,

    /// Require authentication for SSE/WS transports (default: true).
    #[serde(default = "default_transport_auth_required")]
    pub transport_auth_required: bool,

    /// URL of the SurrealDB instance to connect to (e.g., "ws://localhost:8000" or "https://cloud.surrealdb.com").
    /// Note: The builder automatically strips any "ws://" or "wss://" prefixes because the underlying SurrealDB
    /// remote WebSocket connector expects a naked host/port when establishing a connection.
    /// Requires the `surreal` feature flag.
    #[serde(default)]
    pub database_url: Option<String>,
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
            jwt_secret_file: None,
            encoder: default_encoder(),
            websocket: default_websocket(),
            transport_auth_required: default_transport_auth_required(),
            database_url: None,
        }
    }
}

impl BlennyConfig {
    /// Load configuration from environment and a JSON file, falling back to defaults.
    pub fn load() -> Self {
        let mut figment = Figment::new().merge(Env::prefixed("BLENNY_"));

        if std::path::Path::new("blenny.json").exists() {
            figment = figment.merge(Json::file("blenny.json"));
        } else if std::path::Path::new("blenny/blenny.json").exists() {
            figment = figment.merge(Json::file("blenny/blenny.json"));
        } else {
            figment = figment.merge(Json::file("blenny.json"));
        }

        let mut config: Self = figment.extract().unwrap_or_else(|err| {
            eprintln!("Invalid config, using defaults: {err}");
            Self::default()
        });

        // Load JWT secret from file if provided via config or environment
        if let Some(ref file_path) = config.jwt_secret_file {
            match std::fs::read_to_string(file_path) {
                Ok(content) => {
                    config.jwt_secret = content.trim().to_string();
                }
                Err(e) => {
                    eprintln!("Error reading JWT secret file at {file_path}: {e}");
                }
            }
        } else if let Ok(file_path) = std::env::var("BLENNY_JWT_SECRET_FILE") {
            match std::fs::read_to_string(&file_path) {
                Ok(content) => {
                    config.jwt_secret = content.trim().to_string();
                }
                Err(e) => {
                    eprintln!("Error reading JWT secret file from BLENNY_JWT_SECRET_FILE env var at {file_path}: {e}");
                }
            }
        }

        config
    }
}