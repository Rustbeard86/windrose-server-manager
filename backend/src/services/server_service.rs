use chrono::Utc;
use tracing::{error, info, warn};

use crate::config::AppConfig;
use crate::models::{ServerInfo, ServerStatus};
use crate::pid;
use crate::process;
use crate::state::AppState;

/// Attempt to start the managed server process.
///
/// Resolves the executable path from `AppConfig`, spawns the child process,
/// stores the handle in `AppState`, and launches a background watcher task
/// that transitions state to `Crashed` or `Stopped` when the process exits.
pub async fn start(state: &AppState) -> Result<(), String> {
    let current = state.get_server_info().await;
    if current.status == ServerStatus::Running || current.status == ServerStatus::Starting {
        return Err(format!(
            "Server is already {:?}; cannot start",
            current.status
        ));
    }

    let exe_path = state
        .config
        .server_executable
        .as_ref()
        .ok_or_else(|| "server_executable is not configured".to_string())?;

    // Resolve relative paths against the binary directory.
    let exe_path = if exe_path.is_absolute() {
        exe_path.clone()
    } else {
        let base = AppConfig::binary_dir()
            .ok_or_else(|| "Cannot determine binary directory to resolve relative path".to_string())?;
        base.join(exe_path)
    };

    if !exe_path.exists() {
        return Err(format!(
            "Server executable not found: {}",
            exe_path.display()
        ));
    }

    info!("Starting server: {}", exe_path.display());

    state
        .set_server_info(ServerInfo {
            status: ServerStatus::Starting,
            pid: None,
            uptime_seconds: None,
            started_at: None,
        })
        .await;

    // Resolve working_dir relative to the binary directory.
    let working_dir = state.config.server_working_dir.as_ref().map(|d| {
        if d.is_absolute() {
            d.clone()
        } else {
            AppConfig::binary_dir().map(|b| b.join(d)).unwrap_or_else(|| d.clone())
        }
    });

    let managed = process::spawn(
        &exe_path,
        &state.config.server_args,
        working_dir.as_deref(),
    )
    .map_err(|e| format!("Failed to spawn server process: {e}"))?;

    let pid = managed.pid;
    let started_at = Utc::now();

    // Persist the PID so a restarted manager can re-adopt the process.
    pid::write(pid);

    // Store the process handle.
    *state.process.lock().await = Some(managed);

    state
        .set_server_info(ServerInfo {
            status: ServerStatus::Running,
            pid: Some(pid),
            uptime_seconds: Some(0),
            started_at: Some(started_at),
        })
        .await;

    info!(pid, "Server process is running");

    // Spawn a background watcher that updates state when the process exits.
    let watcher_state = state.clone();
    tokio::spawn(async move {
        watch_process(watcher_state, started_at).await;
    });

    Ok(())
}

/// Attempt to stop the managed server process.
///
/// First tries a graceful shutdown (via stdin "stop" command), then waits up
/// to `server_stop_timeout_secs` before force-killing the process.
pub async fn stop(state: &AppState) -> Result<(), String> {
    let current = state.get_server_info().await;
    if current.status == ServerStatus::Stopped || current.status == ServerStatus::Stopping {
        return Err(format!(
            "Server is already {:?}; cannot stop",
            current.status
        ));
    }

    info!("Stopping server (graceful + forced fallback)");

    state
        .set_server_info(ServerInfo {
            status: ServerStatus::Stopping,
            ..current
        })
        .await;

    let timeout = state.config.server_stop_timeout_secs;

    let result = {
        let mut proc_guard = state.process.lock().await;
        match proc_guard.as_mut() {
            None => {
                // No live Child handle — this may be an adopted process from
                // a previous manager session. Kill by PID if we have one.
                if let Some(existing_pid) = current.pid {
                    info!(pid = existing_pid, "Killing adopted server process by PID");
                    pid::kill_by_pid(existing_pid)
                } else {
                    warn!("No tracked process and no PID; transitioning to Stopped");
                    Ok(())
                }
            }
            Some(managed) => {
                // Try graceful shutdown first.
                let sent = managed.graceful_stop().await;
                if sent {
                    info!("Graceful stop command sent; waiting up to {timeout}s");
                } else {
                    warn!("Stdin unavailable; skipping graceful stop, will force-kill");
                }

                // Wait with timeout.
                let wait_result = tokio::time::timeout(
                    std::time::Duration::from_secs(timeout),
                    managed.wait(),
                )
                .await;

                match wait_result {
                    Ok(Ok(status)) => {
                        info!(status = ?status, "Server process exited gracefully");
                        Ok(())
                    }
                    Ok(Err(e)) => {
                        warn!("wait() error: {e}; attempting force kill");
                        managed
                            .kill()
                            .await
                            .map_err(|e| format!("Force kill failed: {e}"))
                    }
                    Err(_elapsed) => {
                        warn!("Graceful stop timed out after {timeout}s; force killing");
                        managed
                            .kill()
                            .await
                            .map_err(|e| format!("Force kill failed: {e}"))
                    }
                }
            }
        }
    };

    // Release the process handle regardless of outcome.
    *state.process.lock().await = None;

    // Remove the PID file — server is being deliberately stopped.
    pid::remove();

    // Clear online players when the server stops.
    state.clear_players().await;

    state
        .set_server_info(ServerInfo {
            status: ServerStatus::Stopped,
            pid: None,
            uptime_seconds: None,
            started_at: None,
        })
        .await;

    result
}

/// Restart the server: stop (with graceful+forced fallback) then start.
pub async fn restart(state: &AppState) -> Result<(), String> {
    let current_status = state.get_server_info().await.status;

    // Prevent overlapping restarts.
    if current_status == ServerStatus::Starting || current_status == ServerStatus::Stopping {
        return Err(format!(
            "Server is transitioning ({:?}); cannot restart now",
            current_status
        ));
    }

    info!("Restarting server");

    if current_status != ServerStatus::Stopped {
        stop(state).await?;
    }
    start(state).await
}

/// Send a command string to the running server's stdin pipe.
///
/// Returns an error if the server is not running or the stdin pipe is
/// unavailable.  See `process.rs` for documentation on Windows stdin
/// limitations.
pub async fn send_command(state: &AppState, command: &str) -> Result<(), String> {
    let current = state.get_server_info().await;
    if current.status != ServerStatus::Running {
        return Err(format!(
            "Server is not running (status: {:?}); cannot send command",
            current.status
        ));
    }

    let mut proc_guard = state.process.lock().await;
    match proc_guard.as_mut() {
        None => Err("No tracked process to send command to".to_string()),
        Some(managed) => managed
            .send_command(command)
            .await
            .map_err(|e| format!("Failed to send command: {e}")),
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Background task: wait for the server process to exit and update state.
async fn watch_process(state: AppState, started_at: chrono::DateTime<Utc>) {
    // The process handle is stored before this task is spawned, so the lock
    // should always yield Some immediately on the first attempt.
    let exit_result = {
        let mut guard = state.process.lock().await;
        match guard.as_mut() {
            Some(managed) => managed.wait().await,
            None => {
                // Should not happen in normal operation; log and exit.
                tracing::warn!("watch_process: no process handle found on entry");
                return;
            }
        }
    };

    // Release the handle now that the process has exited.
    *state.process.lock().await = None;

    // Process exited on its own — remove the PID file.
    pid::remove();

    let current = state.get_server_info().await;
    // Only update if still in Running/Starting (not if the stop path
    // already transitioned the state).
    if current.status == ServerStatus::Running || current.status == ServerStatus::Starting {
        let uptime = Utc::now()
            .signed_duration_since(started_at)
            .num_seconds()
            .max(0) as u64;

        match exit_result {
            Ok(status) if status.success() => {
                info!(status = ?status, uptime, "Server process exited cleanly");
                state
                    .set_server_info(ServerInfo {
                        status: ServerStatus::Stopped,
                        pid: None,
                        uptime_seconds: Some(uptime),
                        started_at: Some(started_at),
                    })
                    .await;
            }
            Ok(status) => {
                error!(status = ?status, uptime, "Server process exited with non-zero status — marking as Crashed");
                state
                    .set_server_info(ServerInfo {
                        status: ServerStatus::Crashed,
                        pid: None,
                        uptime_seconds: Some(uptime),
                        started_at: Some(started_at),
                    })
                    .await;
                state.event_hub.publish(crate::models::WsEvent::Notification {
                    level: "error".to_string(),
                    message: format!("Server crashed (exit status: {status})"),
                });
            }
            Err(e) => {
                error!("Error waiting for server process: {e}");
                state
                    .set_server_info(ServerInfo {
                        status: ServerStatus::Crashed,
                        pid: None,
                        uptime_seconds: Some(uptime),
                        started_at: Some(started_at),
                    })
                    .await;
            }
        }

        // Clear online players on any uncontrolled exit.
        state.clear_players().await;
    }
}

