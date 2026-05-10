extern crate self as blenny;

pub mod auth;          // <-- NEW
pub mod builder;
pub mod conduit;
pub mod embedded;
pub mod module;
pub mod modules;
pub mod transport;

pub use blenny_macros::blenny_module;
pub use blenny_macros::blenny_auth_provider;   // NEW

pub use builder::BlennyBuilder;
pub use conduit::Conduit;
pub use module::{BlennyModule, ModuleRegistration};
pub use transport::TransportHub;
pub use auth::User;                            // so handlers can extract Extension<User>
