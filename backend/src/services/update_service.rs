//! Manager-app update-check service.
//!
//! # Overview
//!
//! This service checks whether a newer version of the Windrose Server Manager
//! is available by querying the GitHub Releases API.  The result is stored in
//! `AppState::update_state` and broadcast over WebSocket.
//!
//! # Current limitations
//!
//! - **Read-only**: this service only checks for updates; it does not download
//!   or apply them.  In-place binary replacement on Windows carries significant
//!   risk (the running executable is locked by the OS) and is therefore deferred
//!   to a future phase.
//! - **Network dependency**: if the host has no internet access (or the GitHub
//!   API is unreachable) the check will fail gracefully with the error logged
//!   and the state set to `Failed`.
//! - **Versioning assumption**: both the current binary version and the GitHub
//!   tag are expected to follow SemVer (`v0.1.0` or `0.1.0`).  A naive
//!   string comparison is used; if the current version equals the latest tag,
//!   no update is reported.
//!
//! # Self-update groundwork
//!
//! A clean update path for a single native binary on Windows would require:
//! 1. Download the new binary alongside the running one (`windrose-server-manager-new.exe`).
//! 2. Spawn a minimal launcher / updater that waits for the manager to exit,
//!    replaces the file, and re-launches it.
//! 3. Manager triggers its own graceful shutdown after spawning the updater.
//!
//! This PR establishes the abstraction (check, state, event) that a future
//! phase can build on without changing the API surface.

use tracing::{info, warn};

use crate::state::AppState;

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
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Kick off a non-blocking update-check task.
///
/// The task hits `AppConfig::update_check_url`, parses the JSON response, and
/// updates `AppState::update_state` accordingly.  Progress is also visible via
/// the `update_available` WebSocket event.
pub fn start_update_check(state: AppState) {
    tokio::spawn(async move {
        run_update_check(state).await;
    });
}

// ---------------------------------------------------------------------------
// Internal
// ---------------------------------------------------------------------------

async fn run_update_check(state: AppState) {
    let url = state.config.update_check_url.clone();
    let current_version = state.get_update_state().await.current_version.clone();

    state.set_update_checking().await;
    info!(url, current_version, "Checking for manager-app updates");

    match fetch_latest_release(&url).await {
        Ok(release) => {
            let latest = normalise_version(&release.tag_name);
            let update_available = latest != normalise_version(&current_version);
            let download_url = release.html_url;
            let release_notes = release.body;

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
            warn!("Update check failed: {e}");
            state.set_update_failed(e).await;
        }
    }
}

/// Strip a leading `v` from a version string for comparison purposes.
fn normalise_version(v: &str) -> String {
    v.trim_start_matches('v').to_string()
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
        return Err(format!(
            "GitHub API returned HTTP {}: {}",
            resp.status().as_u16(),
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
    fn update_detected_when_versions_differ() {
        assert_ne!(normalise_version("v0.2.0"), normalise_version("0.1.0"));
    }

    #[test]
    fn no_update_when_versions_match() {
        assert_eq!(normalise_version("v0.1.0"), normalise_version("0.1.0"));
    }
}
