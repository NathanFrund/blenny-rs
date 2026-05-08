extern crate self as blenny;

use axum::Router;
use std::io;
use std::sync::Arc;
use tera::{Context as TeraContext, Tera};

// Re‑export the proc macro so users only need `use blenny::*`.
pub use blenny_macros::blenny_module;

// ---------------------------------------------------------------------------
// Module discovery
// ---------------------------------------------------------------------------

/// Every Blenny module must implement this trait.
pub trait BlennyModule: Send + Sync + 'static {
    /// Human‑readable name.
    fn name(&self) -> &'static str;
    /// Add this module's routes to the Axum router.
    fn register_routes(&self, router: Router) -> Router;
}

/// A registration record for an auto‑discovered module.
pub struct ModuleRegistration {
    pub name: &'static str,
    pub constructor: fn() -> Box<dyn BlennyModule>,
}

// Tell `inventory` to collect all `ModuleRegistration` objects.
inventory::collect!(ModuleRegistration);

// ---------------------------------------------------------------------------
// Conduit – template engine (Tera backend)
// ---------------------------------------------------------------------------

pub struct Conduit {
    engine: Tera,
}

impl Conduit {
    /// Hot‑reload from a directory (development).
    pub fn hot_reload(template_dir: &str) -> Result<Self, io::Error> {
        let engine = Tera::new(&format!("{}/**/*", template_dir))
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        Ok(Conduit { engine })
    }

    /// Frozen – load templates from embedded directory (production).
    pub fn frozen() -> Result<Self, io::Error> {
        Ok(Conduit {
            engine: Tera::default(), // will be populated from rust-embed later
        })
    }

    pub fn render(&self, template: &str, ctx: &TeraContext) -> Result<String, tera::Error> {
        // Automatically append .tera if no extension is given.
        let template_name = if template.ends_with(".tera") {
            template.to_string()
        } else {
            format!("{}.tera", template)
        };
        self.engine.render(&template_name, ctx)
    }
}

// ---------------------------------------------------------------------------
// BlennyBuilder
// ---------------------------------------------------------------------------

pub struct BlennyBuilder {
    conduit: Option<Arc<Conduit>>,
}

impl Default for BlennyBuilder {
    fn default() -> Self {
        BlennyBuilder { conduit: None }
    }
}

impl BlennyBuilder {
    pub fn with_conduit(mut self, conduit: Conduit) -> Self {
        self.conduit = Some(Arc::new(conduit));
        self
    }

    pub fn with_default_transports(self) -> Self {
        // SSE / WebSocket later
        self
    }

    pub async fn serve(self, addr: &str) -> Result<(), Box<dyn std::error::Error>> {
        let mut router = Router::new();

        // 1. Auto‑discover all modules and collect their routes.
        println!("Discovering modules...");
        let mut module_count = 0;
        for reg in inventory::iter::<ModuleRegistration> {
            let module: Box<dyn BlennyModule> = (reg.constructor)();
            println!("  - registering module: {}", reg.name);
            router = module.register_routes(router);
            module_count += 1;
        }
        println!("Registered {} module(s).", module_count);

        // 2. Add core routes.
        router = router.route("/health", axum::routing::get(|| async { "OK" }));

        // 3. 💉 Apply layers last – they wrap everything above.
        if let Some(conduit) = &self.conduit {
            router = router.layer(axum::Extension(conduit.clone()));
        }

        let listener = tokio::net::TcpListener::bind(addr).await?;
        println!("Blenny server listening on http://{addr}");
        axum::serve(listener, router.into_make_service()).await?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Declare the modules directory
// ---------------------------------------------------------------------------
pub mod modules;
