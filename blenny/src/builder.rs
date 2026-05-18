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

    fn discover_modules(&self) -> Vec<(String, Box<dyn BlennyModule>)> {
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
        module_regs
    }

    fn discover_auth_provider(&self) -> Option<Arc<dyn AuthProvider>> {
        let mut auth_provider: Option<Arc<dyn AuthProvider>> = None;
        for reg in inventory::iter::<AuthRegistration> {
            if auth_provider.is_some() {
                eprintln!("Warning: multiple auth providers found; using the first.");
                break;
            }
            auth_provider = Some((reg.constructor)());
            println!("Using auth provider: {}", reg.name);
        }
        auth_provider
    }

    async fn build_app_state(
        &self,
        auth_provider: Option<Arc<dyn AuthProvider>>,
    ) -> Result<Arc<AppState>, Box<dyn std::error::Error>> {
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

        // ---- Auth public paths ----
        let auth_public_paths: Vec<String> = auth_provider
            .as_ref()
            .map(|a| a.public_paths().into_iter().map(String::from).collect())
            .unwrap_or_default();

        // ---- Encoder ----
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

        // ---- Conduit ----
        let conduit = match &self.conduit {
            Some(c) => c.clone(),
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

        // ---- AppState ----
        #[cfg(feature = "surreal")]
        let app_state = Arc::new(AppState::new(
            conduit,
            self.transport_hub.clone(),
            auth_provider.clone(),
            encoder,
            self.config.jwt_secret.clone(),
            auth_public_paths,
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
            auth_public_paths,
            self.config.clone(),
        ));

        if let Some(sender) = &self.app_state_sender {
            let _ = sender.send(app_state.clone());
        }

        Ok(app_state)
    }

    pub async fn serve(self, addr: &str) -> Result<(), Box<dyn std::error::Error>> {
        let module_regs = self.discover_modules();
        let auth_provider = self.discover_auth_provider();
        let app_state = self.build_app_state(auth_provider.clone()).await?;
        let router = build_router(&app_state, module_regs, self.config.websocket);

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

fn build_router(
    app_state: &Arc<AppState>,
    module_regs: Vec<(String, Box<dyn BlennyModule>)>,
    websocket: bool,
) -> Router {
    // ---- Module lifecycle ----
    let mut protected_router = Router::new();
    let mut active_modules: Vec<Box<dyn BlennyModule>> = Vec::new();
    for (name, mut module) in module_regs {
        module.initialize_module(app_state.clone());
        protected_router = module.register_routes(protected_router);
        println!("  - registered routes for module: {}", name);
        active_modules.push(module);
    }

    for module in &active_modules {
        module.start_module();
    }

    // ---- Build final router ----
    let mut router = Router::new();

    // Public infrastructure routes (no auth)
    router = router.route("/health", axum::routing::get(|| async { "OK" }));
    router = router.route("/sse", axum::routing::get(sse_handler));
    if websocket {
        router = router.route("/ws", axum::routing::get(ws_handler));
    }
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

    router
}
