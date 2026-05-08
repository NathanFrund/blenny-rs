use crate::Conduit; // re‑exported in lib.rs
use crate::module::{BlennyModule, ModuleRegistration};
use axum::Router;
use std::sync::Arc;

pub struct BlennyBuilder {
    pub conduit: Option<Arc<Conduit>>,
    // transport_hub will go here later
}

impl BlennyBuilder {
    pub fn new() -> Self {
        BlennyBuilder { conduit: None }
    }

    pub fn with_conduit(mut self, conduit: Conduit) -> Self {
        self.conduit = Some(Arc::new(conduit));
        self
    }

    pub fn with_default_transports(self) -> Self {
        self
    }

    pub async fn serve(self, addr: &str) -> Result<(), Box<dyn std::error::Error>> {
        let mut router = Router::new();

        // Auto‑discover modules
        println!("Discovering modules...");
        let mut module_count = 0;
        for reg in inventory::iter::<ModuleRegistration> {
            let module: Box<dyn BlennyModule> = (reg.constructor)();
            println!("  - registering module: {}", reg.name);
            router = module.register_routes(router);
            module_count += 1;
        }
        println!("Registered {} module(s).", module_count);

        // Health check
        router = router.route("/health", axum::routing::get(|| async { "OK" }));

        // Apply layers **last**
        if let Some(conduit) = &self.conduit {
            router = router.layer(axum::Extension(conduit.clone()));
        }

        let listener = tokio::net::TcpListener::bind(addr).await?;
        println!("Blenny server listening on http://{addr}");
        axum::serve(listener, router.into_make_service()).await?;
        Ok(())
    }
}

impl Default for BlennyBuilder {
    fn default() -> Self {
        Self::new()
    }
}
