// blenny/src/app_state.rs
use crate::auth::AuthProvider;
use crate::conduit::Conduit;
use crate::encoder::TransportEncoder;
use crate::transport::TransportHub;
use std::collections::HashSet;
use std::sync::Arc;

/// Bundles all framework singletons into one injectable piece.
/// In the future this will become `axum::State<AppState>`.
pub struct AppState {
    pub conduit: Option<Arc<Conduit>>,
    pub hub: Arc<TransportHub>,
    pub auth: Option<Arc<dyn AuthProvider>>,
    pub encoder: Arc<dyn TransportEncoder>,
    pub jwt_secret: String,
    pub public_paths: HashSet<String>,
}

impl AppState {
    pub fn new(
        conduit: Option<Arc<Conduit>>,
        hub: Arc<TransportHub>,
        auth: Option<Arc<dyn AuthProvider>>,
        encoder: Arc<dyn TransportEncoder>,
        jwt_secret: String,
        public_paths: HashSet<String>,
    ) -> Self {
        AppState { conduit, hub, auth, encoder, jwt_secret, public_paths }
    }
}