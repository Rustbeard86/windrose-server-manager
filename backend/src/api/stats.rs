use axum::{extract::State, Json};

use crate::models::{ApiResponse, ServerStats};
use crate::state::AppState;

/// `GET /api/server/stats` — latest collected resource-usage stats.
pub async fn get(State(app): State<AppState>) -> Json<ApiResponse<Option<ServerStats>>> {
    let stats = app.get_server_stats().await;
    Json(ApiResponse::ok(stats))
}
