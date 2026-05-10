// blenny/src/module.rs
use axum::Router;
use std::sync::Arc;

use crate::{Conduit, TransportHub};

pub trait BlennyModule: Send + Sync + 'static {
    fn name(&self) -> &'static str;

    fn is_enabled(&self) -> bool {
        true
    }

    fn initialize_module(&mut self, _conduit: Option<Arc<Conduit>>, _hub: Arc<TransportHub>) {
        // Default: do nothing
        // Auth will be passed via AppState later
    }

    fn start_module(&self) {
        // Default: do nothing
    }

    fn stop_module(&self) {
        // Default: do nothing
    }

    fn register_routes(&self, router: Router) -> Router;
}

pub struct ModuleRegistration {
    pub name: &'static str,
    pub constructor: fn() -> Box<dyn BlennyModule>,
}

inventory::collect!(ModuleRegistration);
