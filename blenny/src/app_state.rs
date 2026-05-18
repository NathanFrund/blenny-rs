// blenny/src/app_state.rs
use crate::auth::AuthProvider;
use crate::conduit::Conduit;
use crate::config::BlennyConfig;
use crate::encoder::TransportEncoder;
use crate::transport::TransportHub;
use std::sync::Arc;

#[cfg(feature = "surreal")]
use surrealdb::Surreal;

#[cfg(feature = "surreal")]
use surrealdb::engine::remote::ws::Client;

/// Bundles all framework singletons into one injectable piece.
/// In the future this will become `axum::State<AppState>`.
pub struct AppState {
    pub conduit: Arc<Conduit>,
    pub hub: Arc<TransportHub>,
    pub auth: Option<Arc<dyn AuthProvider>>,
    pub encoder: Arc<dyn TransportEncoder>,
    pub jwt_secret: String,
    pub config: BlennyConfig,
    #[cfg(feature = "surreal")]
    pub surrealdb: Option<Arc<Surreal<Client>>>,
}

impl AppState {
    pub fn new(
        conduit: Arc<Conduit>,
        hub: Arc<TransportHub>,
        auth: Option<Arc<dyn AuthProvider>>,
        encoder: Arc<dyn TransportEncoder>,
        jwt_secret: String,
        config: BlennyConfig,
        #[cfg(feature = "surreal")] surrealdb: Option<Arc<Surreal<Client>>>,
    ) -> Self {
        AppState {
            conduit,
            hub,
            auth,
            encoder,
            jwt_secret,
            config,
            #[cfg(feature = "surreal")]
            surrealdb,
        }
    }
}
