use axum::{extract::State, http::StatusCode, Json};

use crate::models::{ApiResponse, ServerConfig, WorldConfig};
use crate::services::config_service;
use crate::state::AppState;

/// `GET /api/config/server`
pub async fn get_server_config(
    State(app): State<AppState>,
) -> (StatusCode, Json<ApiResponse<ServerConfig>>) {
    match config_service::load_server_config(&app).await {
        Ok(cfg) => (StatusCode::OK, Json(ApiResponse::ok(cfg))),
        Err(msg) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::err(msg)),
        ),
    }
}

/// `PUT /api/config/server`
pub async fn put_server_config(
    State(app): State<AppState>,
    Json(cfg): Json<ServerConfig>,
) -> (StatusCode, Json<ApiResponse<()>>) {
    match config_service::save_server_config(&app, cfg).await {
        Ok(()) => (StatusCode::OK, Json(ApiResponse::ok(()))),
        Err(msg) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::err(msg)),
        ),
    }
}

/// `GET /api/config/world`
pub async fn get_world_config(
    State(app): State<AppState>,
) -> (StatusCode, Json<ApiResponse<WorldConfig>>) {
    match config_service::load_world_config(&app).await {
        Ok(cfg) => (StatusCode::OK, Json(ApiResponse::ok(cfg))),
        Err(msg) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::err(msg)),
        ),
    }
}

/// `PUT /api/config/world`
pub async fn put_world_config(
    State(app): State<AppState>,
    Json(cfg): Json<WorldConfig>,
) -> (StatusCode, Json<ApiResponse<()>>) {
    match config_service::save_world_config(&app, cfg).await {
        Ok(()) => (StatusCode::OK, Json(ApiResponse::ok(()))),
        Err(msg) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::err(msg)),
        ),
    }
}
