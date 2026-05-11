#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialise crypto provider for jsonwebtoken
    jsonwebtoken::crypto::CryptoProvider::install_default(
        &jsonwebtoken::crypto::aws_lc::DEFAULT_PROVIDER,
    )
    .expect("Failed to install crypto provider");

    // Load configuration (env vars + blenny.json + defaults)
    let config = blenny::BlennyConfig::load();

    // Choose template source
    let conduit = if let Some(dir) = &config.template_dir {
        blenny::Conduit::hot_reload(dir)?
    } else if cfg!(debug_assertions) {
        let template_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/templates");
        blenny::Conduit::hot_reload(template_dir)?
    } else {
        blenny::Conduit::frozen()?
    };

    blenny::BlennyBuilder::new(config)
        .with_conduit(conduit)
        .with_default_transports()
        .serve("0.0.0.0:8081")
        .await
}
