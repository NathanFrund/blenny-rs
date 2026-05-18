// blenny/src/builder.rs
use crate::auth::{AuthProvider, AuthRegistration};
use crate::config::BlennyConfig;
use crate::encoder::TransportEncoder;
use crate::middleware::AntiFragileLayer;
use crate::module::{BlennyModule, ModuleRegistration};
use crate::transport::{TransportHub, sse_handler, ws_handler};
use crate::{AppState, Conduit};
use axum::Router;
use std::sync::Arc;
use tower_http::services::ServeDir;

#[cfg(feature = "surreal")]
use surrealdb::Surreal;
#[cfg(feature = "surreal")]
use surrealdb::engine::remote::ws::Ws;

pub struct BlennyBuilder {
    pub conduit: Option<Arc<Conduit>>,
    pub transport_hub: Arc<TransportHub>,
    config: BlennyConfig,
    app_state_sender: Option<tokio::sync::mpsc::UnboundedSender<Arc<AppState>>>,
}

impl BlennyBuilder {
    pub fn new(config: BlennyConfig) -> Self {
        BlennyBuilder {
            conduit: None,
            transport_hub: Arc::new(TransportHub::new()),
            config,
            app_state_sender: None,
        }
    }

    pub fn with_conduit(mut self, conduit: Conduit) -> Self {
        self.conduit = Some(Arc::new(conduit));
        self
    }

    pub fn with_default_transports(self) -> Self {
        self
    }

    pub fn with_app_state_sender(
        mut self,
        sender: tokio::sync::mpsc::UnboundedSender<Arc<AppState>>,
    ) -> Self {
        self.app_state_sender = Some(sender);
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

        // ---- Optional SurrealDB connection ----
        #[cfg(feature = "surreal")]
        let surrealdb = if let Some(raw_url) = &self.config.database_url {
            let url = raw_url
                .strip_prefix("ws://")
                .or_else(|| raw_url.strip_prefix("wss://"))
                .unwrap_or(raw_url)
                .to_string();

            let db = Surreal::new::<Ws>(url).await?;
            db.signin(surrealdb::opt::auth::Root {
                username: "root".into(),
                password: "root".into(),
            })
            .await?;

            db.use_ns("blenny").use_db("blenny").await?;
            println!("Connected to SurrealDB at {}", raw_url);
            Some(Arc::new(db))
        } else {
            None
        };

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

        let conduit = match self.conduit {
            Some(c) => c,
            None => {
                let default_conduit = if let Some(dir) = &self.config.template_dir {
                    Conduit::hot_reload(dir)?
                } else if cfg!(debug_assertions) {
                    Conduit::hot_reload("templates")?
                } else {
                    Conduit::frozen()?
                };
                Arc::new(default_conduit)
            }
        };

        #[cfg(feature = "surreal")]
        let app_state = Arc::new(AppState::new(
            conduit,
            self.transport_hub.clone(),
            auth_provider.clone(),
            encoder,
            self.config.jwt_secret.clone(),
            self.config.clone(),
            surrealdb,
        ));

        #[cfg(not(feature = "surreal"))]
        let app_state = Arc::new(AppState::new(
            conduit,
            self.transport_hub.clone(),
            auth_provider.clone(),
            encoder,
            self.config.jwt_secret.clone(),
            self.config.clone(),
        ));

        if let Some(sender) = &self.app_state_sender {
            let _ = sender.send(app_state.clone());
        }

        // ---- Auth layer and routes ----
        let mut protected_router = Router::new();
        let mut active_modules: Vec<Box<dyn BlennyModule>> = Vec::new();
        for (name, mut module) in module_regs {
            module.initialize_module(app_state.clone());
            protected_router = module.register_routes(protected_router);
            println!("  - registered routes for module: {}", name);
            active_modules.push(module);
        }

        // Start all modules (they can start background tasks)
        for module in &active_modules {
            module.start_module();
        }

        // ---- Build final router ----
        let mut router = Router::new();

        // Public infrastructure routes (no auth)
        router = router.route("/health", axum::routing::get(|| async { "OK" }));
        router = router.route("/sse", axum::routing::get(sse_handler));
        if self.config.websocket {
            router = router.route("/ws", axum::routing::get(ws_handler));
        }
        // Static assets (dev only)
        #[cfg(debug_assertions)]
        {
            router = router.nest_service("/static", ServeDir::new("static"));
        }

        // Apply auth layer to module routes only
        if let Some(auth) = &app_state.auth {
            protected_router = protected_router.merge(auth.auth_routes());
            protected_router = auth.protect_router(protected_router);
        }

        // Anti-fragile middleware for module routes
        protected_router = protected_router.layer(AntiFragileLayer);

        // Merge protected module routes into the main router
        router = router.merge(protected_router);

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
