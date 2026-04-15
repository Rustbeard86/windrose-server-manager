pub mod backup;
pub mod command;
pub mod config;
pub mod health;
pub mod history;
pub mod install;
pub mod logs;
pub mod players;
pub mod schedule;
pub mod server;
pub mod state;
pub mod update;
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
        // ── Server command input ─────────────────────────────────────────────
        .route("/api/server/command", post(command::handler))
        // ── Configuration ───────────────────────────────────────────────────
        .route("/api/config/server", get(config::get_server_config))
        .route("/api/config/server", put(config::put_server_config))
        .route("/api/config/world", get(config::get_world_config))
        .route("/api/config/world", put(config::put_world_config))
        // ── Logs ────────────────────────────────────────────────────────────
        .route("/api/logs", get(logs::handler))
        // ── Players ─────────────────────────────────────────────────────────
        .route("/api/players", get(players::handler))
        // ── Player / event history ───────────────────────────────────────────
        .route("/api/history/players", get(history::player_events))
        // ── Backup ──────────────────────────────────────────────────────────
        .route("/api/backup", get(backup::get_status))
        .route("/api/backup/create", post(backup::create))
        // ── Scheduled restart ────────────────────────────────────────────────
        .route("/api/schedule", get(schedule::get))
        .route("/api/schedule", put(schedule::put))
        .route("/api/schedule/cancel", post(schedule::cancel))
        // ── Install / detect ─────────────────────────────────────────────────
        .route("/api/install", get(install::get_status))
        .route("/api/install/detect", post(install::detect))
        .route("/api/install/run", post(install::run))
        // ── App update check ─────────────────────────────────────────────────
        .route("/api/update", get(update::get_status))
        .route("/api/update/check", post(update::check))
        // ── WebSocket ───────────────────────────────────────────────────────
        .route("/ws", get(ws::handler))
        // ── Middleware ──────────────────────────────────────────────────────
        .layer(cors)
        .with_state(app_state)
}
