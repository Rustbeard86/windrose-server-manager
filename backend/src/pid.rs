//! PID-file management for the managed server process.
//!
//! The file is written when the server starts and removed when it stops or
//! is explicitly killed through the manager API.  It is intentionally NOT
//! removed when the manager itself exits, so that a restarted manager can
//! reconstruct the server's running state.

use std::path::PathBuf;
use tracing::{info, warn};

/// Return the path to the PID file: `<binary dir>/windrose-server.pid`.
pub fn pid_path() -> Option<PathBuf> {
    Some(
        std::env::current_exe()
            .ok()?
            .parent()?
            .join("windrose-server.pid"),
    )
}

/// Write the server PID to the PID file.
pub fn write(pid: u32) {
    if let Some(path) = pid_path() {
        match std::fs::write(&path, pid.to_string()) {
            Ok(_) => info!(pid, path = %path.display(), "Wrote server PID file"),
            Err(e) => warn!("Could not write PID file {}: {e}", path.display()),
        }
    }
}

/// Remove the PID file. Called only when the server is deliberately stopped
/// via the manager (not when the manager itself exits).
pub fn remove() {
    if let Some(path) = pid_path() {
        if path.exists() {
            if let Err(e) = std::fs::remove_file(&path) {
                warn!("Could not remove PID file {}: {e}", path.display());
            } else {
                info!(path = %path.display(), "Removed server PID file");
            }
        }
    }
}

/// Read the PID from an existing PID file. Returns `None` if absent or
/// unparseable.
pub fn read() -> Option<u32> {
    let path = pid_path()?;
    let text = std::fs::read_to_string(&path).ok()?;
    text.trim().parse::<u32>().ok()
}

/// Kill a process by PID using a platform-appropriate system command.
/// Used to stop an adopted server process that the manager did not spawn
/// in this session.
pub fn kill_by_pid(pid: u32) -> Result<(), String> {
    #[cfg(windows)]
    {
        let out = std::process::Command::new("taskkill")
            .args(["/F", "/PID", &pid.to_string()])
            .output()
            .map_err(|e| format!("taskkill failed: {e}"))?;
        if out.status.success() {
            Ok(())
        } else {
            Err(format!(
                "taskkill exited with {}",
                String::from_utf8_lossy(&out.stderr)
            ))
        }
    }
    #[cfg(not(windows))]
    {
        let out = std::process::Command::new("kill")
            .args(["-9", &pid.to_string()])
            .output()
            .map_err(|e| format!("kill -9 failed: {e}"))?;
        if out.status.success() {
            Ok(())
        } else {
            Err(format!(
                "kill exited with {}",
                String::from_utf8_lossy(&out.stderr)
            ))
        }
    }
}

