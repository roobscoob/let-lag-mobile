use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use tower_http::cors::{Any, CorsLayer};

pub fn create_router(tiles_dir: PathBuf) -> Router {
    let tiles_dir = Arc::new(tiles_dir);

    Router::new()
        .route("/tiles/{z}/{x}/{y}/tile.pbf", get(serve_tile))
        .route("/health", get(health))
        .layer(CorsLayer::new().allow_origin(Any))
        .with_state(tiles_dir)
}

async fn serve_tile(
    State(tiles_dir): State<Arc<PathBuf>>,
    Path((z, x, y)): Path<(u32, u32, u32)>,
) -> Response {
    let tile_path = tiles_dir.join(format!("{z}/{x}/{y}.pbf"));

    match tokio::fs::read(&tile_path).await {
        Ok(bytes) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "application/x-protobuf"),
                (header::CONTENT_ENCODING, "gzip"),
            ],
            bytes,
        )
            .into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn health() -> &'static str {
    "OK"
}
