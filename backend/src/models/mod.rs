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
    #[serde(default)]
    pub server_name: String,
    #[serde(default)]
    pub max_players: u32,
    #[serde(default)]
    pub port: u16,
    #[serde(default)]
    pub invite_code: Option<String>,
    /// Catch-all for unknown fields so round-trip serialisation preserves them.
    #[serde(default, flatten)]
    pub extra: serde_json::Value,
}

/// Represents world / level settings.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorldConfig {
    #[serde(default)]
    pub world_name: String,
    #[serde(default)]
    pub seed: Option<String>,
    /// Catch-all for unknown fields.
    #[serde(default, flatten)]
    pub extra: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Player models
// ---------------------------------------------------------------------------

/// A player currently online or recently seen.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    pub name: String,
    pub joined_at: DateTime<Utc>,
}

/// Kind of player lifecycle event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlayerEventKind {
    Joined,
    Left,
}

/// A timestamped record of a player joining or leaving.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerEvent {
    pub player_name: String,
    pub kind: PlayerEventKind,
    pub timestamp: DateTime<Utc>,
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
    /// Players currently online.
    pub players: Vec<Player>,
    pub player_count: usize,
    /// Recent player join/leave events (bounded ring buffer).
    pub player_events: Vec<PlayerEvent>,
    pub app_version: String,
    pub snapshot_at: DateTime<Utc>,
    /// Backup subsystem status.
    pub backup: BackupStatus,
    /// Scheduled-restart configuration and runtime state.
    pub schedule: ScheduleState,
    /// Install / source-detect subsystem state.
    pub install: InstallState,
    /// App-update check state.
    pub update: UpdateState,
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
// Backup models
// ---------------------------------------------------------------------------

/// Job state for a running or recently-completed backup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BackupJobState {
    #[default]
    Idle,
    Running,
    Done,
    Failed,
}

/// Metadata for a completed backup artifact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupEntry {
    /// Unique identifier for this backup.
    pub id: String,
    pub created_at: DateTime<Utc>,
    /// Filesystem path to the backup directory or archive.
    pub path: String,
    /// Total bytes copied.
    pub size_bytes: u64,
    /// Optional human-readable label.
    pub label: Option<String>,
}

/// Current backup subsystem status (job state + history).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BackupStatus {
    pub job_state: BackupJobState,
    pub progress_pct: Option<u8>,
    pub current_file: Option<String>,
    /// Completed backup entries, oldest first.
    pub history: Vec<BackupEntry>,
    pub last_error: Option<String>,
}

// ---------------------------------------------------------------------------
// Schedule models
// ---------------------------------------------------------------------------

/// Configuration for the daily scheduled-restart feature.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleConfig {
    /// Whether scheduled restarts are enabled.
    pub enabled: bool,
    /// Hour of day (0–23) to initiate the restart sequence.
    pub restart_hour: u8,
    /// Minute (0–59) to initiate the restart sequence.
    pub restart_minute: u8,
    /// Seconds of warning countdown before the restart fires.
    pub warning_seconds: u64,
}

impl Default for ScheduleConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            restart_hour: 4,
            restart_minute: 0,
            warning_seconds: 60,
        }
    }
}

/// Runtime state of the scheduler.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScheduleState {
    pub config: ScheduleConfig,
    /// Whether a warning countdown is currently in progress.
    pub countdown_active: bool,
    /// Seconds remaining in the current countdown (if active).
    pub countdown_seconds_remaining: Option<u64>,
    /// ISO date string (`YYYY-MM-DD`) of the last day a restart fired,
    /// used to prevent double-firing within the same daily window.
    pub last_restart_date: Option<String>,
}

// ---------------------------------------------------------------------------
// Install / detect models
// ---------------------------------------------------------------------------

/// Job state for an in-progress install or detect operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum InstallJobState {
    #[default]
    Idle,
    Detecting,
    Detected,
    Installing,
    Done,
    Failed,
}

/// State of the install / source-detect subsystem.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InstallState {
    pub job_state: InstallJobState,
    pub progress_pct: Option<u8>,
    pub current_file: Option<String>,
    /// Filesystem paths that look like valid Windrose source installs.
    pub detected_sources: Vec<String>,
    /// Destination path for the most recent or in-progress install.
    pub destination: Option<String>,
    pub last_error: Option<String>,
}

// ---------------------------------------------------------------------------
// Update models
// ---------------------------------------------------------------------------

/// State of the app-update check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum UpdateCheckState {
    #[default]
    Idle,
    Checking,
    Done,
    Failed,
}

/// State of the manager-app update subsystem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateState {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub update_available: bool,
    pub last_checked_at: Option<DateTime<Utc>>,
    pub check_state: UpdateCheckState,
    pub release_notes: Option<String>,
    pub download_url: Option<String>,
}

impl Default for UpdateState {
    fn default() -> Self {
        Self {
            current_version: env!("CARGO_PKG_VERSION").to_string(),
            latest_version: None,
            update_available: false,
            last_checked_at: None,
            check_state: UpdateCheckState::Idle,
            release_notes: None,
            download_url: None,
        }
    }
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
    /// Backup job progress or completion.
    BackupProgress {
        job_state: String,
        progress_pct: Option<u8>,
        current_file: Option<String>,
        entry: Option<BackupEntry>,
    },
    /// Scheduled-restart countdown tick or cancellation.
    ScheduleCountdown {
        seconds_remaining: u64,
        cancelled: bool,
    },
    /// Install / copy job progress.
    InstallProgress {
        job_state: String,
        progress_pct: Option<u8>,
        current_file: Option<String>,
    },
    /// Update check result: a newer version is available.
    UpdateAvailable {
        current_version: String,
        latest_version: String,
        download_url: Option<String>,
    },
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

