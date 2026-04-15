use axum::{extract::State, Json};

use crate::models::{ApiResponse, PlayerEvent};
use crate::state::AppState;

/// `GET /api/history/players`
///
/// Returns the bounded ring buffer of recent player join/leave events (oldest
/// first).  Events are retained in memory during the manager's lifetime; if
/// `history_file_path` is configured in `AppConfig` they are also persisted
/// to disk and reloaded on startup.
pub async fn player_events(
    State(app): State<AppState>,
) -> Json<ApiResponse<Vec<PlayerEvent>>> {
    Json(ApiResponse::ok(app.get_player_events().await))
}
