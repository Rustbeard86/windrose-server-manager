use axum::{extract::State, http::StatusCode, Json};

use crate::models::{ApiResponse, UpdateState};
use crate::services::update_service;
use crate::state::AppState;

/// `GET /api/update`
///
/// Returns the current update-check state (last result, check in progress,
/// whether an update is available, etc.).
pub async fn get_status(State(app): State<AppState>) -> Json<ApiResponse<UpdateState>> {
    Json(ApiResponse::ok(app.get_update_state().await))
}

/// `POST /api/update/check`
///
/// Kick off a non-blocking update check against the configured GitHub
/// Releases API endpoint.  Returns `202 Accepted` immediately; the result is
/// published via the `update_available` WebSocket event and is accessible via
/// `GET /api/update`.
pub async fn check(State(app): State<AppState>) -> (StatusCode, Json<ApiResponse<()>>) {
    update_service::start_update_check(app);
    (StatusCode::ACCEPTED, Json(ApiResponse::ok(())))
}
