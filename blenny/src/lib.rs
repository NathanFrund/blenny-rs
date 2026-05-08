// blenny/src/lib.rs
extern crate self as blenny;

pub mod builder;
pub mod conduit;
pub mod module;
pub mod modules; // your modules directory

// Re‑exports for the public API
pub use builder::BlennyBuilder;
pub use conduit::Conduit;
pub use module::{BlennyModule, ModuleRegistration};

// Re‑export the proc macro
pub use blenny_macros::blenny_module;
