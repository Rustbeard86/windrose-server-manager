use axum::{extract::State, http::StatusCode, Json};

use crate::models::{ApiResponse, UpdateState};
use crate::services::update_service;
use crate::state::AppState;

/// `GET /api/update`
pub async fn get_status(State(app): State<AppState>) -> Json<ApiResponse<UpdateState>> {
    Json(ApiResponse::ok(app.get_update_state().await))
}

/// `POST /api/update/check`
///
/// Triggers an update check against the configured GitHub Releases endpoint.
/// Returns 202 immediately; result is available via `GET /api/update` and the
/// `update_available` WebSocket event.
pub async fn check(State(app): State<AppState>) -> (StatusCode, Json<ApiResponse<()>>) {
    if app.config.update_check_url.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::err("Update checks are disabled (update_check_url is empty)")),
        );
    }
    update_service::start_update_check(app);
    (StatusCode::ACCEPTED, Json(ApiResponse::ok(())))
}

/// `POST /api/update/apply`
///
/// Downloads the new binary, extracts the embedded updater script, spawns it
/// detached, then initiates a graceful manager shutdown.  The updater replaces
/// the binary and relaunches the manager.  Returns 202 immediately.
pub async fn apply(State(app): State<AppState>) -> (StatusCode, Json<ApiResponse<()>>) {
    let us = app.get_update_state().await;
    if !us.update_available {
        return (
            StatusCode::CONFLICT,
            Json(ApiResponse::err("No update is available")),
        );
    }
    update_service::start_apply_update(app);
    (StatusCode::ACCEPTED, Json(ApiResponse::ok(())))
}

