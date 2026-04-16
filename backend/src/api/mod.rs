pub mod auth;
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
pub mod setup;
pub mod state;
pub mod stats;
pub mod update;
pub mod ws;

use axum::{
    http::HeaderValue,
    Router,
    middleware,
    routing::{get, post, put},
};
use tower_http::cors::{AllowOrigin, Any, CorsLayer};

use crate::state::AppState;

/// Build the main API router. All routes are prefixed `/api` except the
/// WebSocket endpoint `/ws` and the static file fallback.
pub fn build_router(app_state: AppState) -> Router {
    let cors = if app_state.config.trusted_origins.is_empty() {
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any)
    } else {
        let origins = app_state
            .config
            .trusted_origins
            .iter()
            .filter_map(|raw| match HeaderValue::from_str(raw) {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!(origin = %raw, "Skipping invalid trusted origin: {e}");
                    None
                }
            })
            .collect::<Vec<_>>();

        if origins.is_empty() {
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any)
        } else {
            CorsLayer::new()
                .allow_origin(AllowOrigin::list(origins))
                .allow_credentials(true)
                .allow_methods(Any)
                .allow_headers(Any)
        }
    };

    Router::new()
        // ── Health ──────────────────────────────────────────────────────────
        .route("/api/health", get(health::handler))
        // ── Auth ────────────────────────────────────────────────────────────
        .route("/api/auth/status", get(auth::status))
        .route("/api/auth/bootstrap", post(auth::bootstrap))
        .route("/api/auth/login", post(auth::login))
        .route("/api/auth/logout", post(auth::logout))
        .route("/api/auth/me", get(auth::me))
        .route("/api/auth/users", get(auth::list_users))
        .route("/api/auth/users/:id", put(auth::update_user))
        .route("/api/auth/invites", get(auth::list_invites))
        .route("/api/auth/invites", post(auth::create_invite))
        .route("/api/auth/register", post(auth::register_with_invite))
        .route("/api/auth/reset-code", post(auth::create_reset_code))
        .route("/api/auth/reset-password", post(auth::reset_password))
        .route("/api/auth/audit", get(auth::list_audit))
        // ── App state snapshot ──────────────────────────────────────────────
        .route("/api/state", get(state::handler))
        // ── Server lifecycle ────────────────────────────────────────────────
        .route("/api/server/start", post(server::start))
        .route("/api/server/stop", post(server::stop))
        .route("/api/server/restart", post(server::restart))        // ── Server stats ───────────────────────────────────────────
        .route("/api/server/stats", get(stats::get))        // ── Server command input ─────────────────────────────────────────────
        .route("/api/server/command", post(command::handler))
        // ── Configuration ───────────────────────────────────────────────────
        .route("/api/config/server", get(config::get_server_config))
        .route("/api/config/server", put(config::put_server_config))
        .route("/api/config/world", get(config::get_world_config))
        .route("/api/config/world", put(config::put_world_config))
        // ── Config file management ──────────────────────────────────────────
        .route("/api/config/files", get(config::list_files))
        .route("/api/config/file", get(config::get_file))
        .route("/api/config/file", put(config::put_file))
        .route("/api/config/file/validate", post(config::validate_file))
        .route("/api/config/file/mtime", get(config::get_file_mtime))
        // ── Setup / FTUE ─────────────────────────────────────────────────────
        .route("/api/setup/status", get(setup::status))
        .route("/api/setup/config", put(setup::apply))
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
        .route("/api/update/apply", post(update::apply))
        // ── WebSocket ───────────────────────────────────────────────────────
        .route("/ws", get(ws::handler))
        // ── Middleware ──────────────────────────────────────────────────────
        .layer(middleware::from_fn_with_state(
            app_state.clone(),
            auth::require_auth,
        ))
        .layer(cors)
        .with_state(app_state)
}
