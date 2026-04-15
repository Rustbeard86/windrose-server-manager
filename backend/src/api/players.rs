use axum::{extract::State, Json};

use crate::models::{ApiResponse, Player, PlayerEvent};
use crate::state::AppState;

/// Response body for `GET /api/players`.
#[derive(Debug, serde::Serialize)]
pub struct PlayersResponse {
    pub online: Vec<Player>,
    pub online_count: usize,
    pub recent_events: Vec<PlayerEvent>,
}

/// `GET /api/players`
///
/// Returns the list of currently-online players and recent join/leave events.
pub async fn handler(State(app): State<AppState>) -> Json<ApiResponse<PlayersResponse>> {
    let online = app.get_players().await;
    let online_count = online.len();
    let recent_events = app.get_player_events().await;

    Json(ApiResponse::ok(PlayersResponse {
        online,
        online_count,
        recent_events,
    }))
}
