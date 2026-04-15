use axum::{extract::State, Json};

use crate::models::{ApiResponse, LogLine};
use crate::state::AppState;

/// `GET /api/logs`
///
/// Returns the current contents of the log ring buffer (oldest first).
pub async fn handler(State(app): State<AppState>) -> Json<ApiResponse<Vec<LogLine>>> {
    let lines = app.get_log_snapshot().await;
    Json(ApiResponse::ok(lines))
}
