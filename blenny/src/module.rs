// blenny/src/module.rs
use axum::Router;
use std::sync::Arc;

use crate::app_state::AppState;

pub trait BlennyModule: Send + Sync + 'static {
    fn name(&self) -> &'static str;

    fn is_enabled(&self) -> bool {
        true
    }

    fn initialize_module(&mut self, _state: Arc<AppState>) {
        // Default: do nothing
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
    pub prefix: Option<&'static str>,
}

inventory::collect!(ModuleRegistration);
