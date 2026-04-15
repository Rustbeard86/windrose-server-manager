use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};

use crate::config::AppConfig;
use crate::models::ApiResponse;
use crate::services::install_service;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct SetupStatus {
    pub needs_setup: bool,
    pub config: AppConfig,
    /// Auto-detected server executable (absolute path), if found near the binary.
    pub detected_executable: Option<String>,
    /// Auto-detected working directory, if found.
    pub detected_working_dir: Option<String>,
    /// Auto-detected log file path, if found.
    pub detected_log_file: Option<String>,
}

/// `GET /api/setup/status` — check whether the FTUE wizard should be shown.
pub async fn status(
    State(app): State<AppState>,
) -> (StatusCode, Json<ApiResponse<SetupStatus>>) {
    let needs_setup = !app.config.server_executable_exists();

    let detected = install_service::detect_local_server();

    let status = SetupStatus {
        needs_setup,
        config: (*app.config).clone(),
        detected_executable: detected.as_ref().map(|d| d.executable.to_string_lossy().to_string()),
        detected_working_dir: detected.as_ref().map(|d| d.working_dir.to_string_lossy().to_string()),
        detected_log_file: detected.map(|d| d.log_file_path.to_string_lossy().to_string()),
    };
    (StatusCode::OK, Json(ApiResponse::ok(status)))
}

/// Partial config sent by the FTUE wizard.
#[derive(Debug, Deserialize)]
pub struct SetupApply {
    #[serde(default)]
    pub bind_address: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub server_executable: Option<String>,
    #[serde(default)]
    pub server_working_dir: Option<String>,
    #[serde(default)]
    pub log_file_path: Option<String>,
    #[serde(default)]
    pub server_args: Option<Vec<String>>,
}

/// `PUT /api/setup/config` — merge partial config and persist.
pub async fn apply(
    State(app): State<AppState>,
    Json(req): Json<SetupApply>,
) -> (StatusCode, Json<ApiResponse<AppConfig>>) {
    // Start from the currently-loaded config.
    let mut cfg = (*app.config).clone();

    if let Some(v) = req.bind_address {
        cfg.bind_address = v;
    }
    if let Some(v) = req.port {
        cfg.port = v;
    }
    if let Some(v) = req.server_executable {
        cfg.server_executable = if v.is_empty() { None } else { Some(v.into()) };
    }
    if let Some(v) = req.server_working_dir {
        cfg.server_working_dir = if v.is_empty() { None } else { Some(v.into()) };
    }
    if let Some(v) = req.log_file_path {
        cfg.log_file_path = if v.is_empty() { None } else { Some(v.into()) };
    }
    if let Some(v) = req.server_args {
        cfg.server_args = v;
    }

    // Persist to disk.
    if let Err(e) = cfg.save() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::err(e)),
        );
    }

    (StatusCode::OK, Json(ApiResponse::ok(cfg)))
}
