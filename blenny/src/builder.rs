// blenny/src/builder.rs
use crate::auth::{AuthProvider, AuthRegistration};
use crate::config::BlennyConfig;
use crate::encoder::TransportEncoder;
use crate::middleware::AntiFragileLayer;
use crate::module::{BlennyModule, ModuleRegistration};
use crate::transport::{TransportHub, sse_handler};
use crate::{AppState, Conduit};
use axum::Router;
use std::sync::Arc;
use tower_http::services::ServeDir;

pub struct BlennyBuilder {
    pub conduit: Option<Arc<Conduit>>,
    pub transport_hub: Arc<TransportHub>,
    config: BlennyConfig,
}

impl BlennyBuilder {
    pub fn new(config: BlennyConfig) -> Self {
        BlennyBuilder {
            conduit: None,
            transport_hub: Arc::new(TransportHub::new()),
            config,
        }
    }

    pub fn with_conduit(mut self, conduit: Conduit) -> Self {
        self.conduit = Some(Arc::new(conduit));
        self
    }

    pub fn with_default_transports(self) -> Self {
        self
    }

    pub async fn serve(self, addr: &str) -> Result<(), Box<dyn std::error::Error>> {
        // ---- Discover modules without immediate init ----
        println!("Discovering modules...");
        let mut module_regs: Vec<(String, Box<dyn BlennyModule>)> = Vec::new();
        for reg in inventory::iter::<ModuleRegistration> {
            let module: Box<dyn BlennyModule> = (reg.constructor)();
            if !module.is_enabled() {
                println!("  - skipping disabled module: {}", reg.name);
                continue;
            }
            println!("  - found module: {}", reg.name);
            module_regs.push((reg.name.to_string(), module));
        }
        println!("Found {} module(s).", module_regs.len());

        // ---- Auth discovery ----
        let mut auth_provider: Option<Arc<dyn AuthProvider>> = None;
        for reg in inventory::iter::<AuthRegistration> {
            if auth_provider.is_some() {
                eprintln!("Warning: multiple auth providers found; using the first.");
                break;
            }
            auth_provider = Some((reg.constructor)());
            println!("Using auth provider: {}", reg.name);
        }

        // ---- Collect public routes from modules ----
        let all_public_paths: std::collections::HashSet<String> = module_regs
            .iter()
            .map(|(_, module)| module.public_routes())
            .flatten()
            .collect();

        // ---- Build AppState ----
        let encoder: Arc<dyn TransportEncoder> = {
            #[cfg(feature = "datastar-sse")]
            {
                Arc::new(crate::encoder::DatastarEncoder)
            }
            #[cfg(not(feature = "datastar-sse"))]
            {
                Arc::new(crate::encoder::StandardEncoder)
            }
        };

        let app_state = Arc::new(AppState::new(
            self.conduit.clone(),
            self.transport_hub.clone(),
            auth_provider.clone(),
            encoder,
            self.config.jwt_secret.clone(),
            all_public_paths,
        ));

        // ---- Initialize modules, register routes ----
        let mut active_modules: Vec<Box<dyn BlennyModule>> = Vec::new();
        let mut protected_router = Router::new();
        for (name, mut module) in module_regs {
            module.initialize_module(app_state.clone());
            protected_router = module.register_routes(protected_router);
            println!("  - registered routes for module: {}", name);
            active_modules.push(module);
        }

        // ---- Auth layer and routes ----
        let mut router = protected_router;
        if let Some(auth) = &app_state.auth {
            router = router.merge(auth.auth_routes()); // public auth routes
            router = auth.protect_router(router); // auth module applies its middleware
        }

        // Apply anti-fragile middleware to module routes
        router = router.layer(AntiFragileLayer);

        // Start all modules
        for module in &active_modules {
            module.start_module();
        }

        // Public routes
        router = router.route("/health", axum::routing::get(|| async { "OK" }));
        router = router.route("/sse", axum::routing::get(sse_handler));

        // Static assets
        #[cfg(debug_assertions)]
        {
            router = router.nest_service(
                "/static",
                ServeDir::new("static"),
            );
        }

        // Inject AppState
        router = router.layer(axum::Extension(app_state.clone()));

        let listener = tokio::net::TcpListener::bind(addr).await?;
        println!("Blenny server listening on http://{addr}");
        axum::serve(listener, router.into_make_service()).await?;
        Ok(())
    }
}

impl Default for BlennyBuilder {
    fn default() -> Self {
        Self::new(BlennyConfig::default())
    }
}
