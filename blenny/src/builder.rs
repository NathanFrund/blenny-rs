// blenny/src/builder.rs
use crate::Conduit;
use crate::module::{BlennyModule, ModuleRegistration};
use crate::transport::{TransportHub, sse_handler};
use axum::Router;
use std::sync::Arc; // new

pub struct BlennyBuilder {
    pub conduit: Option<Arc<Conduit>>,
    pub transport_hub: Arc<TransportHub>, // always present
}

impl BlennyBuilder {
    pub fn new() -> Self {
        BlennyBuilder {
            conduit: None,
            transport_hub: Arc::new(TransportHub::new()), // new
        }
    }

    pub fn with_conduit(mut self, conduit: Conduit) -> Self {
        self.conduit = Some(Arc::new(conduit));
        self
    }

    pub fn with_default_transports(self) -> Self {
        // Placeholder for WebSocket sidecar
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

        // Core routes
        router = router.route("/health", axum::routing::get(|| async { "OK" }));

        // Add SSE endpoint and inject TransportHub
        let hub = self.transport_hub.clone();
        router = router
            .route("/sse", axum::routing::get(sse_handler))
            .layer(axum::Extension(hub));

        // Apply Conduit layer last (if present)
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
