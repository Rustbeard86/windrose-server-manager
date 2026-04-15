pub mod config;
pub mod health;
pub mod logs;
pub mod server;
pub mod state;
pub mod ws;

use axum::{Router, routing::{get, post, put}};
use tower_http::cors::{Any, CorsLayer};

use crate::state::AppState;

/// Build the main API router. All routes are prefixed `/api` except the
/// WebSocket endpoint `/ws` and the static file fallback.
pub fn build_router(app_state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        // ── Health ──────────────────────────────────────────────────────────
        .route("/api/health", get(health::handler))
        // ── App state snapshot ──────────────────────────────────────────────
        .route("/api/state", get(state::handler))
        // ── Server lifecycle ────────────────────────────────────────────────
        .route("/api/server/start", post(server::start))
        .route("/api/server/stop", post(server::stop))
        .route("/api/server/restart", post(server::restart))
        // ── Configuration ───────────────────────────────────────────────────
        .route("/api/config/server", get(config::get_server_config))
        .route("/api/config/server", put(config::put_server_config))
        .route("/api/config/world", get(config::get_world_config))
        .route("/api/config/world", put(config::put_world_config))
        // ── Logs ────────────────────────────────────────────────────────────
        .route("/api/logs", get(logs::handler))
        // ── WebSocket ───────────────────────────────────────────────────────
        .route("/ws", get(ws::handler))
        // ── Middleware ──────────────────────────────────────────────────────
        .layer(cors)
        .with_state(app_state)
}
