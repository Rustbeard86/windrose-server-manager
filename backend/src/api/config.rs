use axum::{extract::{Query, State}, http::StatusCode, Json};
use serde::Deserialize;

use crate::models::{ApiResponse, ServerConfig, WorldConfig};
use crate::services::{config_file_service, config_service};
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

// ---------------------------------------------------------------------------
// Config file management endpoints (raw file read/write/discover)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct FilePathQuery {
    pub path: String,
}

/// `GET /api/config/files` — discover known game config files.
pub async fn list_files(
    State(app): State<AppState>,
) -> (StatusCode, Json<ApiResponse<Vec<config_file_service::ConfigFileInfo>>>) {
    let files = config_file_service::discover_config_files(&app.config);
    (StatusCode::OK, Json(ApiResponse::ok(files)))
}

/// `GET /api/config/file?path=<relative>` — read a config file's raw content.
pub async fn get_file(
    State(app): State<AppState>,
    Query(q): Query<FilePathQuery>,
) -> (StatusCode, Json<ApiResponse<config_file_service::ConfigFileContent>>) {
    match config_file_service::read_config_file(&app.config, &q.path) {
        Ok(content) => (StatusCode::OK, Json(ApiResponse::ok(content))),
        Err(msg) => (StatusCode::BAD_REQUEST, Json(ApiResponse::err(msg))),
    }
}

/// `PUT /api/config/file` — write a config file with conflict detection.
pub async fn put_file(
    State(app): State<AppState>,
    Json(req): Json<config_file_service::ConfigFileWrite>,
) -> (StatusCode, Json<serde_json::Value>) {
    match config_file_service::write_config_file(&app.config, &req) {
        Ok(result) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "success": true,
                "data": {
                    "path": result.path,
                    "content": result.content,
                    "last_modified": result.last_modified,
                },
                "message": null
            })),
        ),
        Err(config_file_service::WriteError::InvalidJson(e)) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "success": false,
                "data": null,
                "message": e
            })),
        ),
        Err(config_file_service::WriteError::BadPath(e)) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "success": false,
                "data": null,
                "message": e
            })),
        ),
        Err(config_file_service::WriteError::Conflict { disk_content, disk_mtime }) => (
            StatusCode::CONFLICT,
            Json(serde_json::json!({
                "success": false,
                "data": {
                    "disk_content": disk_content,
                    "disk_mtime": disk_mtime,
                },
                "message": "File was modified on disk since you last loaded it"
            })),
        ),
        Err(config_file_service::WriteError::Io(e)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "success": false,
                "data": null,
                "message": e
            })),
        ),
    }
}

/// `POST /api/config/file/validate` — validate a JSON string.
pub async fn validate_file(
    Json(req): Json<config_file_service::ValidateRequest>,
) -> (StatusCode, Json<config_file_service::ValidateResponse>) {
    match config_file_service::validate_json(&req.content) {
        Ok(_) => (
            StatusCode::OK,
            Json(config_file_service::ValidateResponse {
                valid: true,
                error: None,
            }),
        ),
        Err(e) => (
            StatusCode::OK,
            Json(config_file_service::ValidateResponse {
                valid: false,
                error: Some(e),
            }),
        ),
    }
}

/// `GET /api/config/file/mtime?path=<relative>` — lightweight mtime check.
pub async fn get_file_mtime(
    State(app): State<AppState>,
    Query(q): Query<FilePathQuery>,
) -> (StatusCode, Json<ApiResponse<chrono::DateTime<chrono::Utc>>>) {
    match config_file_service::get_file_mtime(&app.config, &q.path) {
        Ok(mtime) => (StatusCode::OK, Json(ApiResponse::ok(mtime))),
        Err(msg) => (StatusCode::BAD_REQUEST, Json(ApiResponse::err(msg))),
    }
}
