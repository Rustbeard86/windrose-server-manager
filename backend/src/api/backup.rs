use axum::{extract::State, http::StatusCode, Json};
use serde::Deserialize;

use crate::models::{ApiResponse, BackupStatus};
use crate::services::backup_service;
use crate::state::AppState;

/// `GET /api/backup`
///
/// Returns the current backup job state and the in-memory backup history.
pub async fn get_status(State(app): State<AppState>) -> Json<ApiResponse<BackupStatus>> {
    Json(ApiResponse::ok(app.get_backup_status().await))
}

/// Request body for `POST /api/backup/create`.
#[derive(Debug, Deserialize)]
pub struct CreateBackupRequest {
    /// Optional human-readable label for this backup.
    pub label: Option<String>,
}

/// `POST /api/backup/create`
///
/// Kicks off a non-blocking backup of `server_working_dir`.
/// Returns immediately; monitor progress via the backup status endpoint or the
/// `backup_progress` WebSocket event.
pub async fn create(
    State(app): State<AppState>,
    body: Option<Json<CreateBackupRequest>>,
) -> (StatusCode, Json<ApiResponse<()>>) {
    let label = body.and_then(|b| b.0.label);
    match backup_service::start_backup(&app, label).await {
        Ok(()) => (
            StatusCode::ACCEPTED,
            Json(ApiResponse::ok(())),
        ),
        Err(e) => (
            StatusCode::CONFLICT,
            Json(ApiResponse::err(e)),
        ),
    }
}
