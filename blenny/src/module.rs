// blenny/src/module.rs
use axum::Router;
use std::collections::HashSet;
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

    /// Return a set of route paths (e.g., "/test-page") that should be public.
    /// The auth middleware will NOT require a valid JWT for these paths.
    fn public_routes(&self) -> HashSet<String> {
        HashSet::new()
    }
}

pub struct ModuleRegistration {
    pub name: &'static str,
    pub constructor: fn() -> Box<dyn BlennyModule>,
}

inventory::collect!(ModuleRegistration);
