extern crate self as blenny;

pub mod app_state;                             // new
pub mod auth;          // <-- NEW
pub mod builder;
pub mod config;
pub mod conduit;
pub mod embedded;
pub mod encoder;
pub mod module;
pub mod modules;
pub mod transport;

pub use blenny_macros::blenny_module;
pub use blenny_macros::blenny_auth_provider;   // NEW

pub use app_state::AppState;                   // new
pub use builder::BlennyBuilder;
pub use config::BlennyConfig;
pub use conduit::Conduit;
pub use encoder::{StandardEncoder, TransportEncoder};
#[cfg(feature = "datastar-sse")]
pub use encoder::DatastarEncoder;
pub use module::{BlennyModule, ModuleRegistration};
pub use transport::TransportHub;
pub use auth::User;                            // so handlers can extract Extension<User>
