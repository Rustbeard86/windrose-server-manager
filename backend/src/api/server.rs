use axum::{extract::State, http::StatusCode, Json};

use crate::models::ApiResponse;
use crate::services::server_service;
use crate::state::AppState;

/// `POST /api/server/start`
pub async fn start(
    State(app): State<AppState>,
) -> (StatusCode, Json<ApiResponse<()>>) {
    match server_service::start(&app).await {
        Ok(()) => (StatusCode::OK, Json(ApiResponse::ok(()))),
        Err(msg) => (
            StatusCode::CONFLICT,
            Json(ApiResponse::err(msg)),
        ),
    }
}

/// `POST /api/server/stop`
pub async fn stop(
    State(app): State<AppState>,
) -> (StatusCode, Json<ApiResponse<()>>) {
    match server_service::stop(&app).await {
        Ok(()) => (StatusCode::OK, Json(ApiResponse::ok(()))),
        Err(msg) => (
            StatusCode::CONFLICT,
            Json(ApiResponse::err(msg)),
        ),
    }
}

/// `POST /api/server/restart`
pub async fn restart(
    State(app): State<AppState>,
) -> (StatusCode, Json<ApiResponse<()>>) {
    match server_service::restart(&app).await {
        Ok(()) => (StatusCode::OK, Json(ApiResponse::ok(()))),
        Err(msg) => (
            StatusCode::CONFLICT,
            Json(ApiResponse::err(msg)),
        ),
    }
}
