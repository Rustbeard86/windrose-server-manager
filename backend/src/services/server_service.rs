use chrono::Utc;
use tracing::{info, warn};

use crate::models::{ServerInfo, ServerStatus};
use crate::state::AppState;

/// Attempt to start the managed server process.
///
/// This is a scaffold implementation. In a future iteration, this will:
/// 1. Resolve the server executable path from `AppConfig`.
/// 2. Spawn the process with `tokio::process::Command`.
/// 3. Capture stdout/stderr and pipe them to `log_service::ingest_raw`.
/// 4. Store the `Child` handle and `pid` in a dedicated process registry.
/// 5. Monitor for unexpected exits and update state accordingly.
pub async fn start(state: &AppState) -> Result<(), String> {
    let current = state.get_server_info().await;
    if current.status == ServerStatus::Running || current.status == ServerStatus::Starting {
        return Err(format!(
            "Server is already {:?}; cannot start",
            current.status
        ));
    }

    info!("Starting server (scaffold — no process spawned yet)");

    let starting = ServerInfo {
        status: ServerStatus::Starting,
        pid: None,
        uptime_seconds: None,
        started_at: None,
    };
    state.set_server_info(starting).await;

    // TODO: spawn the actual process here.
    // For now we immediately transition to Running to allow UI development.
    let running = ServerInfo {
        status: ServerStatus::Running,
        pid: Some(0),
        uptime_seconds: Some(0),
        started_at: Some(Utc::now()),
    };
    state.set_server_info(running).await;

    Ok(())
}

/// Attempt to stop the managed server process gracefully.
///
/// Future implementation will send a shutdown signal / stdin command, wait
/// for the process to exit, then forcefully kill if it exceeds a timeout.
pub async fn stop(state: &AppState) -> Result<(), String> {
    let current = state.get_server_info().await;
    if current.status == ServerStatus::Stopped || current.status == ServerStatus::Stopping {
        return Err(format!(
            "Server is already {:?}; cannot stop",
            current.status
        ));
    }

    info!("Stopping server (scaffold — no process signal sent yet)");

    let stopping = ServerInfo {
        status: ServerStatus::Stopping,
        ..current
    };
    state.set_server_info(stopping).await;

    // TODO: send graceful shutdown to the process here.
    let stopped = ServerInfo {
        status: ServerStatus::Stopped,
        pid: None,
        uptime_seconds: None,
        started_at: None,
    };
    state.set_server_info(stopped).await;

    Ok(())
}

/// Restart the server by issuing a stop followed by a start.
pub async fn restart(state: &AppState) -> Result<(), String> {
    warn!("Restarting server (scaffold)");
    stop(state).await?;
    start(state).await
}
