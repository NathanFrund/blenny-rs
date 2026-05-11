// blenny/src/static_assets.rs
use axum::{
    body::Body,
    extract::Path,
    http::{header, StatusCode},
    response::IntoResponse,
};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "static/"]
pub struct StaticAssets;

/// Handler for release mode: serves files from the embedded binary.
pub async fn static_handler(Path(path): Path<String>) -> impl IntoResponse {
    match StaticAssets::get(&path) {
        Some(content) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            (
                [(header::CONTENT_TYPE, mime.as_ref())],
                Body::from(content.data),
            )
                .into_response()
        }
        None => (StatusCode::NOT_FOUND, "File not found").into_response(),
    }
}