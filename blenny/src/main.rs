#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Use the path to the `templates/` directory inside the `blenny` crate.
    let template_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/templates");

    let conduit = if cfg!(debug_assertions) {
        blenny::Conduit::hot_reload(template_dir)?
    } else {
        blenny::Conduit::frozen()?
    };

    blenny::BlennyBuilder::default()
        .with_conduit(conduit)
        .with_default_transports()
        .serve("0.0.0.0:8081")
        .await
}
