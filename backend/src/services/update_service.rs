//! Manager-app update service.

use tracing::{info, warn};

use crate::models::{UpdateApplyState, WsEvent};
use crate::state::AppState;

// ── File name constants ────────────────────────────────────────────────────

const NEW_BINARY_SUFFIX: &str = "windrose-server-manager-new.exe";
const UPDATER_SCRIPT_NAME: &str = "windrose-manager-updater.bat";

/// Embedded updater batch script.  Spawned detached after the new binary is
/// downloaded; waits for the manager process to exit, replaces the binary,
/// relaunches it, then deletes itself.
///
/// Arguments: %1 = manager PID, %2 = new binary path, %3 = current binary path.
const UPDATER_BAT: &str = r#"@echo off
:wait
tasklist /FI "PID eq %1" 2>NUL | find /I "%1" >NUL
if not errorlevel 1 (
    timeout /t 1 /nobreak >NUL
    goto wait
)
move /Y "%~2" "%~3"
start "" "%~3"
del "%~0"
"#;

/// Remove any leftover artefacts created by a previous self-update attempt.
/// Called once on manager startup.
pub fn cleanup_updater_artefacts() {
    let dir = match std::env::current_exe().ok().and_then(|p| {
        p.parent().map(std::path::Path::to_path_buf)
    }) {
        Some(d) => d,
        None => return,
    };
    for name in [NEW_BINARY_SUFFIX, UPDATER_SCRIPT_NAME] {
        let path = dir.join(name);
        if path.exists() {
            if let Err(e) = std::fs::remove_file(&path) {
                warn!("Could not remove updater artefact {}: {e}", path.display());
            } else {
                info!("Cleaned up updater artefact: {}", path.display());
            }
        }
    }
}



// ---------------------------------------------------------------------------
// GitHub API response shape
// ---------------------------------------------------------------------------

/// Minimal subset of the GitHub `/repos/:owner/:repo/releases/latest` response.
#[derive(Debug, serde::Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    html_url: Option<String>,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(Debug, serde::Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Kick off a non-blocking update-check task.
pub fn start_update_check(state: AppState) {
    tokio::spawn(async move {
        run_update_check(state).await;
    });
}

/// Kick off a non-blocking self-update apply task.
///
/// Downloads the new binary from the release, extracts the embedded updater
/// script alongside the binary, spawns the updater detached, then initiates
/// a graceful manager shutdown.  The updater waits for the manager to exit,
/// replaces the binary, and relaunches it.  A restarted manager will clean up
/// any leftover artefacts on startup.
pub fn start_apply_update(state: AppState) {
    tokio::spawn(async move {
        if let Err(e) = run_apply_update(state.clone()).await {
            warn!("Self-update failed: {e}");
            state.set_update_apply_state(UpdateApplyState::Failed).await;
        }
    });
}

async fn run_apply_update(state: AppState) -> Result<(), String> {
    let download_url = {
        let us = state.get_update_state().await;
        if !us.update_available {
            return Err("No update available".to_string());
        }
        if us.apply_state != UpdateApplyState::Idle && us.apply_state != UpdateApplyState::Failed {
            return Err("Apply already in progress".to_string());
        }
        // Prefer release asset URL; fall back to the html_url page.
        us.download_url.ok_or("No download URL available")?
    };

    state.set_update_apply_state(UpdateApplyState::Downloading).await;

    // Determine paths.
    let current_exe = std::env::current_exe()
        .map_err(|e| format!("Cannot determine current binary path: {e}"))?;
    let dir = current_exe
        .parent()
        .ok_or("Cannot determine binary directory")?;
    let new_binary_path = dir.join(NEW_BINARY_SUFFIX);
    let updater_path = dir.join(UPDATER_SCRIPT_NAME);

    // Download new binary.
    info!(url = %download_url, "Downloading update");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .user_agent(concat!("windrose-server-manager/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    let bytes = client
        .get(&download_url)
        .send()
        .await
        .map_err(|e| format!("Download failed: {e}"))?
        .error_for_status()
        .map_err(|e| format!("Download error status: {e}"))?
        .bytes()
        .await
        .map_err(|e| format!("Failed to read download body: {e}"))?;

    tokio::fs::write(&new_binary_path, &bytes)
        .await
        .map_err(|e| format!("Failed to write new binary: {e}"))?;

    info!(path = %new_binary_path.display(), bytes = bytes.len(), "New binary written");

    state.set_update_apply_state(UpdateApplyState::Applying).await;

    // Write the embedded updater script.
    std::fs::write(&updater_path, UPDATER_BAT)
        .map_err(|e| format!("Failed to write updater script: {e}"))?;

    // Get current manager PID.
    let manager_pid = std::process::id();

    // Spawn the updater detached so it survives the manager exiting.
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x00000008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
        std::process::Command::new("cmd")
            .args([
                "/C",
                updater_path.to_str().unwrap_or_default(),
                &manager_pid.to_string(),
                new_binary_path.to_str().unwrap_or_default(),
                current_exe.to_str().unwrap_or_default(),
            ])
            .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
            .spawn()
            .map_err(|e| format!("Failed to spawn updater: {e}"))?;
    }
    #[cfg(not(windows))]
    {
        // Non-Windows: shell script equivalent (no embedded script used).
        let script = format!(
            "#!/bin/sh\nwhile kill -0 {manager_pid} 2>/dev/null; do sleep 1; done\nmv -f '{}' '{}'\n'{}' &\nrm -- \"$0\"\n",
            new_binary_path.display(),
            current_exe.display(),
            current_exe.display()
        );
        std::fs::write(&updater_path, &script)
            .map_err(|e| format!("Failed to write updater script: {e}"))?;
        std::process::Command::new("sh")
            .arg(&updater_path)
            .spawn()
            .map_err(|e| format!("Failed to spawn updater: {e}"))?;
    }

    info!("Updater spawned (PID {manager_pid}); initiating manager shutdown");
    state.set_update_apply_state(UpdateApplyState::PendingRestart).await;

    // Broadcast shutdown event then send Ctrl+C to ourselves.
    state.event_hub.publish(WsEvent::Notification {
        level: "info".to_string(),
        message: "Manager update in progress — will restart automatically.".to_string(),
    });

    // Brief delay so the WS event reaches clients before the socket closes.
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Trigger the graceful shutdown path (same as Ctrl+C).
    #[cfg(windows)]
    {
        unsafe { windows_ctrl_c() };
    }
    #[cfg(not(windows))]
    {
        let _ = nix_raise_sigint();
    }

    Ok(())
}

/// Raise SIGINT on the current process (Unix).
#[cfg(not(windows))]
fn nix_raise_sigint() -> i32 {
    unsafe { libc_raise(2) } // SIGINT = 2
}
#[cfg(not(windows))]
extern "C" {
    #[link_name = "raise"]
    fn libc_raise(sig: i32) -> i32;
}

/// Send a Ctrl+C event to the current console process group (Windows).
#[cfg(windows)]
unsafe fn windows_ctrl_c() {
    // GenerateConsoleCtrlEvent(CTRL_C_EVENT=0, processGroupId=0 → current group)
    windows_generate_console_ctrl_event(0, 0);
}
#[cfg(windows)]
extern "system" {
    #[link_name = "GenerateConsoleCtrlEvent"]
    fn windows_generate_console_ctrl_event(dw_ctrl_event: u32, dw_process_group_id: u32) -> i32;
}



// ---------------------------------------------------------------------------
// Internal
// ---------------------------------------------------------------------------

async fn run_update_check(state: AppState) {
    let url = state.config.update_check_url.clone();

    if url.is_empty() {
        tracing::info!("Update check disabled (update_check_url is empty)");
        return;
    }

    let current_version = state.get_update_state().await.current_version.clone();

    state.set_update_checking().await;
    info!(url, current_version, "Checking for manager-app updates");

    match fetch_latest_release(&url).await {
        Ok(release) => {
            let latest = normalise_version(&release.tag_name);
            let update_available = is_newer_version(&current_version, &release.tag_name);
            let release_notes = release.body;

            // Pick the correct binary asset from the release.
            // Release workflow names assets: windrose-server-manager-{tag}-windows-x64.exe
            let download_url = release
                .assets
                .iter()
                .find(|a| a.name.contains("windows") && a.name.ends_with(".exe"))
                .map(|a| a.browser_download_url.clone())
                .or(release.html_url);

            if update_available {
                info!(
                    current = current_version,
                    latest,
                    "Update available"
                );
            } else {
                info!(version = current_version, "Manager is up to date");
            }

            state
                .set_update_result(
                    latest,
                    update_available,
                    release_notes,
                    download_url,
                )
                .await;
        }
        Err(e) => {
            if e.contains("No releases published yet") {
                info!("{e}");
            } else {
                warn!("Update check failed: {e}");
            }
            state.set_update_failed(e).await;
        }
    }
}

/// Strip a leading `v` from a version string for comparison purposes.
fn normalise_version(v: &str) -> String {
    v.trim_start_matches('v').to_string()
}

/// Returns `true` if `latest` is strictly newer than `current` according to
/// a simple `MAJOR.MINOR.PATCH` comparison.  Falls back to string inequality
/// for non-standard version strings (pre-release suffixes, etc.).
///
/// This avoids false positives like `"0.10.0" > "0.9.0"` being reported
/// incorrectly by a pure lexicographic string comparison.
fn is_newer_version(current: &str, latest: &str) -> bool {
    let parse_semver = |v: &str| -> Option<(u32, u32, u32)> {
        // Accept an optional leading `v` and strip pre-release suffixes.
        let v = v.trim_start_matches('v');
        let base = v.split('-').next().unwrap_or(v);
        let parts: Vec<&str> = base.split('.').collect();
        if parts.len() < 3 {
            return None;
        }
        let major = parts[0].parse::<u32>().ok()?;
        let minor = parts[1].parse::<u32>().ok()?;
        let patch = parts[2].parse::<u32>().ok()?;
        Some((major, minor, patch))
    };

    match (parse_semver(current), parse_semver(latest)) {
        (Some(cur), Some(lat)) => lat > cur,
        // Fall back to string inequality as a safe default.
        _ => normalise_version(latest) != normalise_version(current),
    }
}

/// Perform the HTTP GET and deserialise the GitHub release JSON.
///
/// Uses `reqwest` with a short timeout so a slow/unreachable endpoint does not
/// hang the manager indefinitely.
async fn fetch_latest_release(url: &str) -> Result<GhRelease, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .user_agent(concat!(
            "windrose-server-manager/",
            env!("CARGO_PKG_VERSION")
        ))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))?;

    let resp = client
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        if status == 404 {
            return Err("No update available".to_string());
        }
        return Err(format!(
            "GitHub API returned HTTP {}: {}",
            status,
            resp.status().canonical_reason().unwrap_or("Unknown")
        ));
    }

    resp.json::<GhRelease>()
        .await
        .map_err(|e| format!("Failed to parse GitHub release JSON: {e}"))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalise_strips_leading_v() {
        assert_eq!(normalise_version("v0.2.0"), "0.2.0");
        assert_eq!(normalise_version("0.2.0"), "0.2.0");
        assert_eq!(normalise_version("v1.0.0-beta"), "1.0.0-beta");
    }

    #[test]
    fn update_detected_when_latest_is_newer() {
        assert!(is_newer_version("0.1.0", "0.2.0"));
        assert!(is_newer_version("0.1.0", "v0.2.0"));
        assert!(is_newer_version("0.9.0", "0.10.0")); // catches lexicographic bug
        assert!(is_newer_version("1.0.0", "2.0.0"));
    }

    #[test]
    fn no_update_when_versions_match() {
        assert!(!is_newer_version("0.1.0", "v0.1.0"));
        assert!(!is_newer_version("0.1.0", "0.1.0"));
    }

    #[test]
    fn no_update_when_current_is_newer() {
        // Edge case: local build is ahead of published release.
        assert!(!is_newer_version("0.2.0", "0.1.0"));
        assert!(!is_newer_version("0.10.0", "0.9.0"));
    }
}
