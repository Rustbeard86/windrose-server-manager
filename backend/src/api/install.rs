use std::path::PathBuf;

use axum::{extract::State, http::StatusCode, Json};
use serde::Deserialize;

use crate::models::{ApiResponse, InstallState};
use crate::services::install_service;
use crate::state::AppState;

/// `GET /api/install`
///
/// Returns the current install / detect job state.
pub async fn get_status(State(app): State<AppState>) -> Json<ApiResponse<InstallState>> {
    Json(ApiResponse::ok(app.get_install_state().await))
}

/// `POST /api/install/detect`
///
/// Kick off a background source-detection scan.  The scan probes common Steam
/// library paths for Windrose game directories and updates the install state.
/// Results are also surfaced via the `install_progress` WebSocket event.
pub async fn detect(State(app): State<AppState>) -> (StatusCode, Json<ApiResponse<()>>) {
    install_service::start_detect(app);
    (StatusCode::ACCEPTED, Json(ApiResponse::ok(())))
}

/// Request body for `POST /api/install/run`.
#[derive(Debug, Deserialize)]
pub struct RunInstallRequest {
    /// Source directory containing the server files to copy.
    pub source: String,
    /// Destination directory to install the server files into.
    pub destination: String,
}

/// `POST /api/install/run`
///
/// Kick off a background install (directory copy) from `source` to
/// `destination`.  Returns `202 Accepted` immediately; track progress via the
/// install status endpoint or the `install_progress` WebSocket event.
pub async fn run(
    State(app): State<AppState>,
    Json(body): Json<RunInstallRequest>,
) -> (StatusCode, Json<ApiResponse<()>>) {
    if body.source.is_empty() || body.destination.is_empty() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(ApiResponse::err("source and destination must not be empty")),
        );
    }

    let source = PathBuf::from(&body.source);
    let destination = PathBuf::from(&body.destination);

    if !source.is_absolute() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(ApiResponse::err("source must be an absolute path")),
        );
    }
    if !destination.is_absolute() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(ApiResponse::err("destination must be an absolute path")),
        );
    }

    match install_service::start_install(&app, source, destination).await {
        Ok(()) => (StatusCode::ACCEPTED, Json(ApiResponse::ok(()))),
        Err(e) => (StatusCode::CONFLICT, Json(ApiResponse::err(e))),
    }
}
