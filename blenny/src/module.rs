// blenny/src/module.rs
use axum::Router;

pub trait BlennyModule: Send + Sync + 'static {
    fn name(&self) -> &'static str;
    fn register_routes(&self, router: Router) -> Router;
}

pub struct ModuleRegistration {
    pub name: &'static str,
    pub constructor: fn() -> Box<dyn BlennyModule>,
}

inventory::collect!(ModuleRegistration);
