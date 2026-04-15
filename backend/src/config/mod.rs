use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Top-level application configuration.
///
/// Values can be overridden via environment variables or a future config file.
/// Defaults are chosen to be safe for local-only use.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Address to bind the HTTP server on. Defaults to localhost only.
    pub bind_address: String,
    /// Port to listen on.
    pub port: u16,
    /// Optional path to the managed game-server executable.
    pub server_executable: Option<PathBuf>,
    /// Optional extra arguments to pass to the server executable.
    pub server_args: Vec<String>,
    /// Optional path to the server working directory.
    pub server_working_dir: Option<PathBuf>,
    /// Optional path to the server log file to tail.
    ///
    /// On Windows the log is typically `R5.log` in the server's data directory.
    /// The manager will open this file with shared-read/shared-write access so
    /// it can be read while the server holds it open for writing.
    pub log_file_path: Option<PathBuf>,
    /// Maximum number of log lines held in the ring buffer.
    pub log_buffer_capacity: usize,
    /// Seconds to wait for a graceful shutdown before force-killing the process.
    pub server_stop_timeout_secs: u64,
    /// Maximum number of player events retained in the ring buffer.
    pub player_event_capacity: usize,
    // ── Phase 3 additions ──────────────────────────────────────────────────
    /// Directory where backup artifacts are stored.
    /// Defaults to `./backups` relative to the working directory.
    pub backup_dir: PathBuf,
    /// Optional path for persisting the player-event history ring buffer across
    /// restarts.  When set, the manager reads this file on startup and writes it
    /// whenever a player event fires.
    pub history_file_path: Option<PathBuf>,
    /// GitHub Releases API URL used to check for manager-app updates.
    /// Override to point at a private mirror or a different repo.
    pub update_check_url: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            bind_address: "127.0.0.1".to_string(),
            port: 8787,
            server_executable: None,
            server_args: Vec::new(),
            server_working_dir: None,
            log_file_path: None,
            log_buffer_capacity: 500,
            server_stop_timeout_secs: 15,
            player_event_capacity: 200,
            backup_dir: PathBuf::from("backups"),
            history_file_path: None,
            update_check_url: "https://api.github.com/repos/Rustbeard86/windrose-server-manager/releases/latest".to_string(),
        }
    }
}

impl AppConfig {
    /// Return the full socket address string, e.g. `"127.0.0.1:8787"`.
    pub fn socket_addr(&self) -> String {
        format!("{}:{}", self.bind_address, self.port)
    }
}
