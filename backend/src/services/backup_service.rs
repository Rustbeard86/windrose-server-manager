//! Backup creation and history management.
//!
//! # Overview
//!
//! A backup copies all files from the configured `server_working_dir` to a
//! timestamped subdirectory inside `backup_dir`.  The copy runs in a background
//! tokio task so the HTTP handler returns immediately; progress is broadcast
//! over WebSocket as `backup_progress` events.
//!
//! ## Backup layout
//!
//! ```text
//! backup_dir/
//! └── 20240115_043000/          ← YYYYMMDD_HHMMSS timestamp
//!     ├── ServerDescription.json
//!     ├── WorldDescription.json
//!     └── Saves/
//!         └── ...
//! ```
//!
//! ## Limitations
//!
//! - Only files within `server_working_dir` are backed up.  If save data lives
//!   in a separate directory you must set `server_working_dir` to the common
//!   parent that contains both.
//! - Symbolic links are not followed; only regular files are copied.
//! - No archive/ZIP compression is performed — raw directory copy only.
//! - Backup history is stored in memory only; it resets when the manager
//!   restarts.  A future iteration will persist this index to disk.

use std::path::{Path, PathBuf};

use chrono::Utc;
use tracing::{error, info};
use uuid::Uuid;

use crate::models::{BackupEntry, WsEvent};
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Kick off a non-blocking backup task.
///
/// Returns an error immediately if a backup is already running or if
/// `server_working_dir` is not configured.
pub async fn start_backup(state: &AppState, label: Option<String>) -> Result<(), String> {
    if state.get_backup_status().await.job_state == crate::models::BackupJobState::Running {
        return Err("A backup is already in progress".to_string());
    }

    let source = state
        .config
        .server_working_dir
        .as_ref()
        .ok_or_else(|| {
            "server_working_dir is not configured; cannot determine what to back up".to_string()
        })?
        .clone();

    let state_clone = state.clone();
    tokio::spawn(async move {
        run_backup(state_clone, source, label).await;
    });

    Ok(())
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

async fn run_backup(state: AppState, source: PathBuf, label: Option<String>) {
    let backup_dir = state.config.backup_dir.clone();

    // Ensure the backup root directory exists.
    if let Err(e) = tokio::fs::create_dir_all(&backup_dir).await {
        let msg = format!(
            "Failed to create backup directory {}: {e}",
            backup_dir.display()
        );
        error!("{}", msg);
        state.set_backup_error(msg).await;
        return;
    }

    // Build timestamped destination path.
    let ts = Utc::now().format("%Y%m%d_%H%M%S").to_string();
    let dest = backup_dir.join(&ts);

    if let Err(e) = tokio::fs::create_dir_all(&dest).await {
        let msg = format!("Failed to create backup destination {}: {e}", dest.display());
        error!("{}", msg);
        state.set_backup_error(msg).await;
        return;
    }

    state.set_backup_running(None, None).await;
    info!(source = %source.display(), dest = %dest.display(), "Backup started");

    match copy_dir_recursive(&source, &dest, &state).await {
        Ok(size_bytes) => {
            let entry = BackupEntry {
                id: Uuid::new_v4().to_string(),
                created_at: Utc::now(),
                path: dest.to_string_lossy().to_string(),
                size_bytes,
                label,
            };
            info!(
                path = %dest.display(),
                size_bytes,
                "Backup completed successfully"
            );
            state.finish_backup(entry).await;
        }
        Err(e) => {
            error!("Backup failed: {e}");
            state.set_backup_error(e).await;
        }
    }
}

/// Recursively copy all files from `src` into `dst`.
///
/// Returns the total number of bytes copied on success.
/// Broadcasts a `BackupProgress` WS event for each file copied.
async fn copy_dir_recursive(
    src: &Path,
    dst: &Path,
    state: &AppState,
) -> Result<u64, String> {
    let mut total_bytes = 0u64;

    // Iterative DFS via an explicit stack to avoid unbounded recursion depth.
    let mut stack: Vec<(PathBuf, PathBuf)> = vec![(src.to_path_buf(), dst.to_path_buf())];

    while let Some((from_dir, to_dir)) = stack.pop() {
        let mut read_dir = tokio::fs::read_dir(&from_dir).await.map_err(|e| {
            format!("Cannot read directory {}: {e}", from_dir.display())
        })?;

        while let Some(entry) = read_dir
            .next_entry()
            .await
            .map_err(|e| format!("Directory entry error: {e}"))?
        {
            let ft = entry
                .file_type()
                .await
                .map_err(|e| format!("file_type error: {e}"))?;

            let from_path = entry.path();
            let to_path = to_dir.join(entry.file_name());

            if ft.is_dir() {
                tokio::fs::create_dir_all(&to_path).await.map_err(|e| {
                    format!("create_dir_all {}: {e}", to_path.display())
                })?;
                stack.push((from_path, to_path));
            } else if ft.is_file() {
                let file_name = from_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();

                // Announce which file we are copying.
                state
                    .set_backup_running(None, Some(file_name.clone()))
                    .await;
                state.event_hub.publish(WsEvent::BackupProgress {
                    job_state: "running".to_string(),
                    progress_pct: None,
                    current_file: Some(file_name),
                    entry: None,
                });

                let n = tokio::fs::copy(&from_path, &to_path).await.map_err(|e| {
                    format!(
                        "copy {} → {}: {e}",
                        from_path.display(),
                        to_path.display()
                    )
                })?;
                total_bytes += n;
            }
            // Symbolic links are intentionally skipped.
        }
    }

    Ok(total_bytes)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[tokio::test]
    async fn copies_directory_tree() {
        let tmp = std::env::temp_dir().join(format!(
            "wsm_backup_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let src_dir = tmp.join("src");
        let dst_dir = tmp.join("dst");
        fs::create_dir_all(&src_dir).unwrap();
        fs::create_dir_all(&dst_dir).unwrap();
        fs::write(src_dir.join("a.txt"), b"hello").unwrap();
        fs::create_dir(src_dir.join("sub")).unwrap();
        fs::write(src_dir.join("sub").join("b.txt"), b"world").unwrap();

        let cfg = crate::config::AppConfig {
            server_working_dir: Some(src_dir.clone()),
            backup_dir: dst_dir.clone(),
            ..Default::default()
        };
        let state = crate::state::AppState::new(cfg);

        let bytes = copy_dir_recursive(&src_dir, &dst_dir, &state)
            .await
            .expect("copy should succeed");

        // "hello" (5) + "world" (5) = 10 bytes.
        assert_eq!(bytes, 10);
        assert!(dst_dir.join("a.txt").exists());
        assert!(dst_dir.join("sub").join("b.txt").exists());

        // Cleanup.
        let _ = fs::remove_dir_all(&tmp);
    }
}
