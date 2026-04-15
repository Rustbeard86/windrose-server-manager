//! Embedded static-asset serving for the Windrose Server Manager.
//!
//! At compile time `rust-embed` reads every file under `../static/` (relative
//! to the crate root) and bakes them into the binary.  At runtime the
//! [`EmbeddedAssets`] axum service serves those bytes directly from memory —
//! no filesystem access required.
//!
//! ### SPA routing
//! Any request path that does not match a known asset falls back to
//! `/index.html` so that client-side React Router (or simple hash-routing)
//! works correctly.

use axum::{
    body::Body,
    http::{header, Request, Response, StatusCode},
};
use rust_embed::RustEmbed;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tower::Service;

// ---------------------------------------------------------------------------
// Embedded file corpus
// ---------------------------------------------------------------------------

/// All files produced by `npm run build` (Vite output), embedded at compile
/// time.  The `folder` path is relative to `backend/Cargo.toml`.
#[derive(RustEmbed)]
#[folder = "../static/"]
struct Assets;

// ---------------------------------------------------------------------------
// Content-Type lookup
// ---------------------------------------------------------------------------

fn content_type(path: &str) -> &'static str {
    // Determine by extension; fall back to octet-stream.
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "application/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "eot" => "application/vnd.ms-fontobject",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "txt" | "text" => "text/plain; charset=utf-8",
        "xml" => "application/xml; charset=utf-8",
        "map" => "application/json; charset=utf-8",
        _ => "application/octet-stream",
    }
}

// ---------------------------------------------------------------------------
// Serve a single embedded file by logical path
// ---------------------------------------------------------------------------

fn serve_embedded(asset_path: &str) -> Option<Response<Body>> {
    let file = Assets::get(asset_path)?;
    let mime = content_type(asset_path);

    // Hashed assets (e.g. `assets/index-ABC123.js`) are immutable; serve
    // them with a long-lived cache.  Everything else gets no-cache so the
    // browser always revalidates.
    let cache = if asset_path.starts_with("assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };

    let body = Body::from(file.data.into_owned());
    Some(
        Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime)
            .header(header::CACHE_CONTROL, cache)
            .body(body)
            .expect("valid response"),
    )
}

// ---------------------------------------------------------------------------
// Axum Service — EmbeddedAssetsService
// ---------------------------------------------------------------------------

/// An axum [`Service`] that serves the embedded frontend assets.
///
/// Routing logic:
/// - `GET /`         → `index.html`
/// - `GET /foo/bar`  → try `foo/bar` first; if not found, serve `index.html`
///                     (SPA fallback so React Router works)
/// - Any path ending with a known static-asset extension that is not found
///   returns a proper 404.
#[derive(Clone)]
pub struct EmbeddedAssetsService;

impl<B> Service<Request<B>> for EmbeddedAssetsService {
    type Response = Response<Body>;
    type Error = std::convert::Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<B>) -> Self::Future {
        let raw_path = req.uri().path();
        let raw_asset_path = raw_path.trim_start_matches('/');
        // Track whether this was a root request so we don't misclassify
        // the normalised "index.html" path as a missing static asset.
        let is_root = raw_asset_path.is_empty();
        let asset_path = if is_root { "index.html" } else { raw_asset_path };

        let response = if let Some(resp) = serve_embedded(asset_path) {
            resp
        } else if !is_root && is_static_asset(asset_path) {
            // A specific static asset was requested and not found — 404.
            Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from("Not found"))
                .expect("valid response")
        } else {
            // Unknown path or root: SPA fallback — serve index.html so
            // client-side routing can handle it.
            serve_embedded("index.html").unwrap_or_else(|| {
                Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Body::from("index.html not embedded"))
                    .expect("valid response")
            })
        };

        Box::pin(async move { Ok(response) })
    }
}

/// Returns `true` for paths that look like specific static files (have a file
/// extension) so we can serve a real 404 instead of the SPA shell.
fn is_static_asset(path: &str) -> bool {
    // If the last path segment contains a dot it's almost certainly a file.
    path.rsplit('/').next().map_or(false, |seg| seg.contains('.'))
}
