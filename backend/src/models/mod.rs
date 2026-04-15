use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Server status
// ---------------------------------------------------------------------------

/// Lifecycle state of the managed server process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ServerStatus {
    #[default]
    Stopped,
    Starting,
    Running,
    Stopping,
    Crashed,
}

/// Snapshot of server process information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub status: ServerStatus,
    pub pid: Option<u32>,
    pub uptime_seconds: Option<u64>,
    pub started_at: Option<DateTime<Utc>>,
}

impl Default for ServerInfo {
    fn default() -> Self {
        Self {
            status: ServerStatus::Stopped,
            pid: None,
            uptime_seconds: None,
            started_at: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Config models
// ---------------------------------------------------------------------------

/// Represents the managed server's primary configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServerConfig {
    pub server_name: String,
    pub max_players: u32,
    pub port: u16,
    pub invite_code: Option<String>,
    pub extra: serde_json::Value,
}

/// Represents world / level settings.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorldConfig {
    pub world_name: String,
    pub seed: Option<String>,
    pub extra: serde_json::Value,
}

// ---------------------------------------------------------------------------
// App state snapshot
// ---------------------------------------------------------------------------

/// Full application state snapshot returned by `GET /api/state`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppStateSnapshot {
    pub server: ServerInfo,
    pub server_config: Option<ServerConfig>,
    pub world_config: Option<WorldConfig>,
    pub recent_logs: Vec<LogLine>,
    pub player_count: usize,
    pub app_version: String,
    pub snapshot_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Log models
// ---------------------------------------------------------------------------

/// A single parsed log line.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogLine {
    pub timestamp: DateTime<Utc>,
    pub level: LogLevel,
    pub message: String,
    pub raw: String,
}

/// Severity level of a log line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum LogLevel {
    Info,
    Warn,
    Error,
    Debug,
    Unknown,
}

// ---------------------------------------------------------------------------
// WebSocket event envelope
// ---------------------------------------------------------------------------

/// Event types broadcast over the WebSocket channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", content = "data", rename_all = "snake_case")]
pub enum WsEvent {
    /// Server lifecycle changed.
    ServerStatusChanged(ServerInfo),
    /// A new log line arrived.
    LogLine(LogLine),
    /// A player joined the server.
    PlayerJoined { player_name: String },
    /// A player left the server.
    PlayerLeft { player_name: String },
    /// Generic notification message.
    Notification { level: String, message: String },
    /// Periodic ping to keep connections alive.
    Ping,
}

// ---------------------------------------------------------------------------
// API response wrappers
// ---------------------------------------------------------------------------

/// Standard success/error envelope returned by the API.
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    pub data: Option<T>,
    pub message: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            message: None,
        }
    }

    pub fn err(msg: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            message: Some(msg.into()),
        }
    }
}
