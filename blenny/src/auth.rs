// blenny/src/auth.rs
use axum::Router;
use std::sync::Arc;

// ---------- JWT Claims & User ----------

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct Claims {
    pub sub: String,         // user id
    pub exp: usize,          // expiry timestamp
}

#[derive(Debug, Clone)]
pub struct User {
    pub id: String,
}

// ---------- AuthProvider trait ----------

pub trait AuthProvider: Send + Sync {
    /// Login/logout routes (unprotected)
    fn auth_routes(&self) -> Router;

    /// Apply protection layer to the router
    fn protect_router(&self, router: Router) -> Router;
}

// ---------- Inventory registration ----------

pub struct AuthRegistration {
    pub name: &'static str,
    pub constructor: fn() -> Arc<dyn AuthProvider>,
}

inventory::collect!(AuthRegistration);