// blenny/src/app_state.rs
use crate::auth::AuthProvider;
use crate::conduit::Conduit;
use crate::transport::TransportHub;
use std::sync::Arc;

/// Bundles all framework singletons into one injectable piece.
/// In the future this will become `axum::State<AppState>`.
#[derive(Clone)]
pub struct AppState {
    pub conduit: Option<Arc<Conduit>>,
    pub hub: Arc<TransportHub>,
    pub auth: Option<Arc<dyn AuthProvider>>,
}

impl AppState {
    pub fn new(
        conduit: Option<Arc<Conduit>>,
        hub: Arc<TransportHub>,
        auth: Option<Arc<dyn AuthProvider>>,
    ) -> Self {
        AppState { conduit, hub, auth }
    }
}