use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use futures::{sink::SinkExt, stream::StreamExt};
use tokio::sync::broadcast;
use tracing::{debug, info, warn};

use crate::state::AppState;

/// `GET /ws`
///
/// Upgrades the HTTP connection to a WebSocket. The handler subscribes to the
/// application-wide event broadcast channel and forwards all events to the
/// client as JSON text frames.
///
/// Future iterations will also accept incoming messages from the client
/// (e.g. console commands, ping/pong).
pub async fn handler(
    ws: WebSocketUpgrade,
    headers: HeaderMap,
    State(app): State<AppState>,
) -> impl IntoResponse {
    match crate::api::auth::validate_ws_auth(&app, &headers) {
        Ok(Some(_)) => {}
        Ok(None) => return StatusCode::UNAUTHORIZED.into_response(),
        Err(e) => {
            tracing::warn!("WS auth validation failed: {e}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    }
    ws.on_upgrade(move |socket| handle_socket(socket, app))
}

async fn handle_socket(socket: WebSocket, app: AppState) {
    let mut rx = app.event_hub.subscribe();
    let (mut sender, mut receiver) = socket.split();

    info!("WebSocket client connected");

    // Spawn a task that forwards broadcast events to the WS client.
    let mut send_task = tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    let payload = match serde_json::to_string(&event) {
                        Ok(json) => json,
                        Err(e) => {
                            warn!("Failed to serialise WS event: {e}");
                            continue;
                        }
                    };
                    if sender.send(Message::Text(payload.into())).await.is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!("WS subscriber lagged by {n} messages");
                }
                Err(broadcast::error::RecvError::Closed) => {
                    break;
                }
            }
        }
    });

    // Receive incoming messages from the client (ping / future commands).
    let mut recv_task = tokio::spawn(async move {
        while let Some(result) = receiver.next().await {
            match result {
                Ok(Message::Close(_)) => {
                    debug!("WS client sent close frame");
                    break;
                }
                Ok(msg) => {
                    debug!("WS received: {:?}", msg);
                }
                Err(e) => {
                    warn!("WS receive error: {e}");
                    break;
                }
            }
        }
    });

    // Abort the other task when either task finishes.
    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    }

    info!("WebSocket client disconnected");
}
