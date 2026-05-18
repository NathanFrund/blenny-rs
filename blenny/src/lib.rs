extern crate self as blenny;

pub mod app_state; // new
pub mod auth; // <-- NEW
pub mod builder;
pub mod conduit;
pub mod config;
pub mod embedded;
pub mod encoder;
pub mod error;
pub mod middleware;
pub mod module;
pub mod modules;
pub mod static_assets;
pub mod transport;

pub use blenny_macros::blenny_auth_provider;
pub use blenny_macros::blenny_module; // NEW

pub use app_state::AppState; // new
pub use auth::User;
pub use builder::BlennyBuilder;
pub use conduit::Conduit;
pub use config::BlennyConfig;
#[cfg(feature = "datastar-sse")]
pub use encoder::DatastarEncoder;
pub use encoder::{StandardEncoder, TransportEncoder};
pub use module::{BlennyModule, ModuleRegistration};
pub use transport::TransportHub; // so handlers can extract Extension<User>
