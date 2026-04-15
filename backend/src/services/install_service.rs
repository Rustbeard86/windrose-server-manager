//! Install / source-detect service.
//!
//! # Overview
//!
//! This service handles two related workflows:
//!
//! 1. **Source detection** — probes common Steam library paths on Windows for a
//!    Windrose game / server installation and returns a list of candidate paths.
//!
//! 2. **Install (copy) workflow** — copies server files from a detected (or
//!    manually specified) source directory to a configured destination.
//!
//! Both operations run in background tokio tasks and report progress through
//! the `AppState::install_state` field and `install_progress` WebSocket events.
//!
//! # Windows path detection
//!
//! On Windows the service checks a hard-coded list of common Steam library
//! roots and returns any subdirectory whose name contains "windrose" or
//! "Windrose".  A future enhancement could query the Windows registry
//! (`HKLM\SOFTWARE\WOW6432Node\Valve\Steam`) for the actual Steam install path
//! and parse `libraryfolders.vdf` for additional library roots.
//!
//! On non-Windows hosts none of the default paths exist, so detection returns
//! an empty list.  A user can still trigger an install by specifying an
//! explicit source path.
//!
//! # Limitations
//!
//! - No integrity / checksum verification is performed after the copy.
//! - The install workflow is a simple recursive directory copy; no uninstall /
//!   cleanup of the destination is performed before copying.  Re-running an
//!   install over an existing directory will overwrite matching files.
//! - Progress percentage is not yet computed (file count not pre-enumerated).

use std::path::{Path, PathBuf};

use tracing::{error, info, warn};

use crate::models::{InstallJobState, WsEvent};
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Constants — default Steam library roots to probe
// ---------------------------------------------------------------------------

/// Candidate Steam `steamapps/common` roots.
///
/// The list covers the most common Windows configurations.  Paths that do not
/// exist on the current machine are silently skipped.
const STEAM_COMMON_ROOTS: &[&str] = &[
    r"C:\Program Files (x86)\Steam\steamapps\common",
    r"C:\Program Files\Steam\steamapps\common",
    r"D:\Steam\steamapps\common",
    r"D:\SteamLibrary\steamapps\common",
    r"E:\Steam\steamapps\common",
    r"E:\SteamLibrary\steamapps\common",
    r"F:\Steam\steamapps\common",
    r"F:\SteamLibrary\steamapps\common",
];

/// Substring fragments considered indicative of a Windrose installation.
const WINDROSE_FRAGMENTS: &[&str] = &["windrose", "Windrose"];

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Kick off a background source-detection task.
///
/// Updates `AppState::install_state` and broadcasts `install_progress` events.
pub fn start_detect(state: AppState) {
    tokio::spawn(async move {
        run_detect(state).await;
    });
}

/// Kick off a background install/copy task.
///
/// `source` — directory containing the server files to copy.
/// `destination` — target directory (will be created if it doesn't exist).
///
/// Returns an error immediately if an install is already in progress.
pub async fn start_install(
    state: &AppState,
    source: PathBuf,
    destination: PathBuf,
) -> Result<(), String> {
    let current = state.get_install_state().await;
    if current.job_state == InstallJobState::Installing {
        return Err("An install is already in progress".to_string());
    }

    let state_clone = state.clone();
    tokio::spawn(async move {
        run_install(state_clone, source, destination).await;
    });

    Ok(())
}

// ---------------------------------------------------------------------------
// Detection
// ---------------------------------------------------------------------------

async fn run_detect(state: AppState) {
    state
        .set_install_state(InstallJobState::Detecting, None, None, None)
        .await;
    state.event_hub.publish(WsEvent::InstallProgress {
        job_state: "detecting".to_string(),
        progress_pct: None,
        current_file: None,
    });

    let sources = detect_sources().await;

    if sources.is_empty() {
        warn!("No Windrose source directories found in known Steam paths");
    } else {
        info!("Detected {} Windrose source(s)", sources.len());
        for p in &sources {
            info!(path = %p.display(), "Detected source");
        }
    }

    let source_strings: Vec<String> = sources
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    state.set_install_detected(source_strings).await;
    state.event_hub.publish(WsEvent::InstallProgress {
        job_state: "detected".to_string(),
        progress_pct: None,
        current_file: None,
    });
}

/// Probe known Steam library roots and return directories that match the
/// Windrose fragment list.
pub async fn detect_sources() -> Vec<PathBuf> {
    let mut found = Vec::new();

    for root in STEAM_COMMON_ROOTS {
        let base = Path::new(root);
        if !base.exists() {
            continue;
        }

        match tokio::fs::read_dir(base).await {
            Ok(mut dir) => {
                while let Ok(Some(entry)) = dir.next_entry().await {
                    let name = entry.file_name();
                    let name_str = name.to_string_lossy();
                    if WINDROSE_FRAGMENTS
                        .iter()
                        .any(|f| name_str.to_lowercase().contains(&f.to_lowercase()))
                    {
                        found.push(entry.path());
                    }
                }
            }
            Err(_) => continue,
        }
    }

    found
}

// ---------------------------------------------------------------------------
// Install copy
// ---------------------------------------------------------------------------

async fn run_install(state: AppState, source: PathBuf, destination: PathBuf) {
    info!(
        source = %source.display(),
        dest = %destination.display(),
        "Install started"
    );

    state
        .set_install_state(
            InstallJobState::Installing,
            Some(0),
            None,
            Some(destination.to_string_lossy().to_string()),
        )
        .await;
    state.event_hub.publish(WsEvent::InstallProgress {
        job_state: "installing".to_string(),
        progress_pct: Some(0),
        current_file: None,
    });

    if let Err(e) = tokio::fs::create_dir_all(&destination).await {
        let msg = format!(
            "Failed to create destination directory {}: {e}",
            destination.display()
        );
        error!("{}", msg);
        state.set_install_error(msg).await;
        state.event_hub.publish(WsEvent::InstallProgress {
            job_state: "failed".to_string(),
            progress_pct: None,
            current_file: None,
        });
        return;
    }

    match copy_dir(&source, &destination, &state).await {
        Ok(()) => {
            info!(dest = %destination.display(), "Install completed");
            state
                .set_install_state(InstallJobState::Done, Some(100), None, None)
                .await;
            state.event_hub.publish(WsEvent::InstallProgress {
                job_state: "done".to_string(),
                progress_pct: Some(100),
                current_file: None,
            });
        }
        Err(e) => {
            error!("Install failed: {e}");
            state.set_install_error(e).await;
            state.event_hub.publish(WsEvent::InstallProgress {
                job_state: "failed".to_string(),
                progress_pct: None,
                current_file: None,
            });
        }
    }
}

/// Recursively copy all files from `src` to `dst`.
async fn copy_dir(src: &Path, dst: &Path, state: &AppState) -> Result<(), String> {
    let mut stack: Vec<(PathBuf, PathBuf)> = vec![(src.to_path_buf(), dst.to_path_buf())];

    while let Some((from_dir, to_dir)) = stack.pop() {
        let mut read_dir = tokio::fs::read_dir(&from_dir)
            .await
            .map_err(|e| format!("read_dir {}: {e}", from_dir.display()))?;

        while let Some(entry) = read_dir
            .next_entry()
            .await
            .map_err(|e| format!("directory entry error: {e}"))?
        {
            let ft = entry
                .file_type()
                .await
                .map_err(|e| format!("file_type error: {e}"))?;
            let from_path = entry.path();
            let to_path = to_dir.join(entry.file_name());

            if ft.is_dir() {
                tokio::fs::create_dir_all(&to_path)
                    .await
                    .map_err(|e| format!("create_dir_all {}: {e}", to_path.display()))?;
                stack.push((from_path, to_path));
            } else if ft.is_file() {
                let file_name = from_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();

                state
                    .set_install_state(InstallJobState::Installing, None, Some(file_name.clone()), None)
                    .await;
                state.event_hub.publish(WsEvent::InstallProgress {
                    job_state: "installing".to_string(),
                    progress_pct: None,
                    current_file: Some(file_name),
                });

                tokio::fs::copy(&from_path, &to_path).await.map_err(|e| {
                    format!(
                        "copy {} → {}: {e}",
                        from_path.display(),
                        to_path.display()
                    )
                })?;
            }
            // Symbolic links are intentionally skipped.
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn detect_sources_returns_empty_on_non_windows() {
        // On Linux/macOS none of the Windows Steam paths exist, so the result
        // must be empty.
        #[cfg(not(windows))]
        {
            let sources = detect_sources().await;
            assert!(
                sources.is_empty(),
                "Expected no sources on non-Windows, got: {sources:?}"
            );
        }
        // On Windows the test is a no-op (paths may or may not exist).
        #[cfg(windows)]
        {
            // Just ensure the function doesn't panic.
            let _ = detect_sources().await;
        }
    }
}
