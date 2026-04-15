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
    /// Directory from which static frontend assets are served.
    pub static_dir: PathBuf,
    /// Optional path to the managed game-server executable.
    pub server_executable: Option<PathBuf>,
    /// Optional path to the server working directory.
    pub server_working_dir: Option<PathBuf>,
    /// Maximum number of log lines held in the ring buffer.
    pub log_buffer_capacity: usize,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            bind_address: "127.0.0.1".to_string(),
            port: 8787,
            static_dir: PathBuf::from("static"),
            server_executable: None,
            server_working_dir: None,
            log_buffer_capacity: 500,
        }
    }
}

impl AppConfig {
    /// Return the full socket address string, e.g. `"127.0.0.1:8787"`.
    pub fn socket_addr(&self) -> String {
        format!("{}:{}", self.bind_address, self.port)
    }
}
