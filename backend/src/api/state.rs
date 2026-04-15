use axum::{extract::State, Json};

use crate::models::{ApiResponse, AppStateSnapshot};
use crate::state::AppState;

/// `GET /api/state`
///
/// Returns a full snapshot of the current application state. The frontend
/// should call this on load and after reconnects to rehydrate its local store.
pub async fn handler(State(app): State<AppState>) -> Json<ApiResponse<AppStateSnapshot>> {
    let snapshot = app.snapshot().await;
    Json(ApiResponse::ok(snapshot))
}
