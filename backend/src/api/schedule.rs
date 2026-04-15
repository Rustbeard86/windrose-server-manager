use axum::{extract::State, http::StatusCode, Json};

use crate::models::{ApiResponse, ScheduleConfig, ScheduleState};
use crate::services::schedule_service;
use crate::state::AppState;

/// `GET /api/schedule`
///
/// Returns the current schedule configuration and runtime state.
pub async fn get(State(app): State<AppState>) -> Json<ApiResponse<ScheduleState>> {
    Json(ApiResponse::ok(app.get_schedule_state().await))
}

/// `PUT /api/schedule`
///
/// Update the scheduled-restart configuration.  The scheduler background task
/// picks up the change on its next tick (≤ 30 s).
pub async fn put(
    State(app): State<AppState>,
    Json(cfg): Json<ScheduleConfig>,
) -> (StatusCode, Json<ApiResponse<ScheduleState>>) {
    // Basic validation.
    if cfg.restart_hour > 23 {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(ApiResponse::err("restart_hour must be 0–23")),
        );
    }
    if cfg.restart_minute > 59 {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(ApiResponse::err("restart_minute must be 0–59")),
        );
    }

    app.set_schedule_config(cfg.clone()).await;

    // Persist schedule settings to the config file.
    let mut disk_cfg = (*app.config).clone();
    disk_cfg.schedule_enabled = cfg.enabled;
    disk_cfg.schedule_restart_hour = cfg.restart_hour;
    disk_cfg.schedule_restart_minute = cfg.restart_minute;
    disk_cfg.schedule_warning_seconds = cfg.warning_seconds;
    if let Err(e) = disk_cfg.save() {
        tracing::warn!("Failed to persist schedule config: {e}");
    }

    (
        StatusCode::OK,
        Json(ApiResponse::ok(app.get_schedule_state().await)),
    )
}

/// `POST /api/schedule/cancel`
///
/// Cancel an in-progress restart countdown.  Has no effect if no countdown is
/// currently running.
pub async fn cancel(State(app): State<AppState>) -> Json<ApiResponse<()>> {
    schedule_service::cancel_countdown(&app).await;
    Json(ApiResponse::ok(()))
}
