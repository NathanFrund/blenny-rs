// blenny/src/builder.rs
use crate::Conduit;
use crate::module::{BlennyModule, ModuleRegistration};
use crate::transport::{TransportHub, sse_handler};
use crate::auth::{AuthProvider, AuthRegistration};   // NEW
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
        let mut modules: Vec<Box<dyn BlennyModule>> = Vec::new();

        // Auto‑discover modules
        println!("Discovering modules...");
        let mut module_count = 0;
        for reg in inventory::iter::<ModuleRegistration> {
            let mut module: Box<dyn BlennyModule> = (reg.constructor)();
            if !module.is_enabled() {
                println!("  - skipping disabled module: {}", reg.name);
                continue;
            }
            println!("  - initializing module: {}", reg.name);

            // Initialize module with dependencies
            module.initialize_module(self.conduit.clone(), self.transport_hub.clone());

            // Register routes
            router = module.register_routes(router);
            modules.push(module);
            module_count += 1;
        }
        println!("Registered {} module(s).", module_count);

        // ---- Auth discovery and layer application (NEW) ----
        let mut auth_provider: Option<Arc<dyn AuthProvider>> = None;
        for reg in inventory::iter::<AuthRegistration> {
            if auth_provider.is_some() {
                eprintln!("Warning: multiple auth providers found; using the first.");
                break;
            }
            auth_provider = Some((reg.constructor)());
            println!("Using auth provider: {}", reg.name);
        }

        // Apply protect layer BEFORE auth routes (so login is public)
        if let Some(auth) = &auth_provider {
            router = auth.protect_router(router);
        }

        // Merge auth routes (login, etc.) - they are unprotected
        if let Some(auth) = &auth_provider {
            router = router.merge(auth.auth_routes());
        }
        // ---- end auth ----

        // Start all modules after routes are assembled
        for module in &modules {
            module.start_module();
        }

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
