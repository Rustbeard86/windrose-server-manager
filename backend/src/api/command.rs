use axum::{extract::State, http::StatusCode, Json};
use serde::Deserialize;

use crate::models::ApiResponse;
use crate::services::server_service;
use crate::state::AppState;

/// Request body for `POST /api/server/command`.
#[derive(Debug, Deserialize)]
pub struct CommandRequest {
    pub command: String,
}

/// `POST /api/server/command`
///
/// Forward a text command to the running server via stdin pipe.
///
/// # Windows stdin caveat
/// Whether the server process reads from its stdin pipe depends on the server
/// implementation.  Many Windows game servers read from the Windows console
/// input buffer rather than the Win32 stdin handle, so commands may be silently
/// ignored.  See `process.rs` for a full discussion of the limitation.
pub async fn handler(
    State(app): State<AppState>,
    Json(body): Json<CommandRequest>,
) -> (StatusCode, Json<ApiResponse<()>>) {
    let cmd = body.command.trim().to_string();
    if cmd.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::err("command must not be empty")),
        );
    }

    match server_service::send_command(&app, &cmd).await {
        Ok(()) => (StatusCode::OK, Json(ApiResponse::ok(()))),
        Err(msg) => (StatusCode::CONFLICT, Json(ApiResponse::err(msg))),
    }
}
