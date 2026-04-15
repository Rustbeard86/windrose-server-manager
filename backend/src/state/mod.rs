use chrono::Utc;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex, RwLock};

use crate::config::AppConfig;
use crate::models::{
    AppStateSnapshot, BackupEntry, BackupJobState, BackupStatus, InstallJobState, InstallState,
    LogLine, Player, PlayerEvent, PlayerEventKind, ScheduleState, ServerConfig, ServerInfo,
    UpdateCheckState, UpdateState, WsEvent, WorldConfig,
};
use crate::process::ManagedProcess;

/// Capacity of the WebSocket broadcast channel.
const WS_BROADCAST_CAPACITY: usize = 128;

// ---------------------------------------------------------------------------
// EventHub — broadcast channel for WebSocket events
// ---------------------------------------------------------------------------

/// Cloneable handle to the application-wide WebSocket event broadcast channel.
#[derive(Clone, Debug)]
pub struct EventHub {
    tx: broadcast::Sender<WsEvent>,
}

impl EventHub {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(WS_BROADCAST_CAPACITY);
        Self { tx }
    }

    /// Subscribe to all future events.
    pub fn subscribe(&self) -> broadcast::Receiver<WsEvent> {
        self.tx.subscribe()
    }

    /// Publish an event to all connected WebSocket clients.
    /// Silently drops the event if there are no subscribers.
    pub fn publish(&self, event: WsEvent) {
        let _ = self.tx.send(event);
    }
}

impl Default for EventHub {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Inner mutable state
// ---------------------------------------------------------------------------

/// Inner mutable state, protected by an `RwLock`.
#[derive(Debug)]
struct Inner {
    server_info: ServerInfo,
    server_config: Option<ServerConfig>,
    world_config: Option<WorldConfig>,
    log_buffer: VecDeque<LogLine>,
    log_capacity: usize,
    /// Players currently online, keyed by name.
    players: HashMap<String, Player>,
    /// Bounded ring buffer of recent player join/leave events.
    player_events: VecDeque<PlayerEvent>,
    player_event_capacity: usize,
    // ── Phase 3 ─────────────────────────────────────────────────────────────
    backup_status: BackupStatus,
    schedule_state: ScheduleState,
    install_state: InstallState,
    update_state: UpdateState,
}

// ---------------------------------------------------------------------------
// AppState — central shared state container
// ---------------------------------------------------------------------------

/// Shared application state accessible from all request handlers and services.
///
/// Clone cheaply — all clones share the same underlying data via `Arc`.
#[derive(Clone, Debug)]
pub struct AppState {
    inner: Arc<RwLock<Inner>>,
    pub event_hub: EventHub,
    pub config: Arc<AppConfig>,
    /// The currently-running server process, if any.
    ///
    /// Stored outside the `RwLock<Inner>` to avoid holding the state lock
    /// during potentially-blocking process operations.
    pub process: Arc<Mutex<Option<ManagedProcess>>>,
    /// Flag set to `true` to cancel an in-progress restart countdown.
    pub schedule_cancel: Arc<AtomicBool>,
}

impl AppState {
    pub fn new(config: AppConfig) -> Self {
        let log_capacity = config.log_buffer_capacity;
        let player_event_capacity = config.player_event_capacity;
        Self {
            inner: Arc::new(RwLock::new(Inner {
                server_info: ServerInfo::default(),
                server_config: None,
                world_config: None,
                log_buffer: VecDeque::with_capacity(log_capacity),
                log_capacity,
                players: HashMap::new(),
                player_events: VecDeque::with_capacity(player_event_capacity),
                player_event_capacity,
                backup_status: BackupStatus::default(),
                schedule_state: ScheduleState::default(),
                install_state: InstallState::default(),
                update_state: UpdateState::default(),
            })),
            event_hub: EventHub::new(),
            config: Arc::new(config),
            process: Arc::new(Mutex::new(None)),
            schedule_cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    // -----------------------------------------------------------------------
    // Server info
    // -----------------------------------------------------------------------

    pub async fn get_server_info(&self) -> ServerInfo {
        self.inner.read().await.server_info.clone()
    }

    pub async fn set_server_info(&self, info: ServerInfo) {
        self.inner.write().await.server_info = info.clone();
        self.event_hub
            .publish(WsEvent::ServerStatusChanged(info));
    }

    // -----------------------------------------------------------------------
    // Config
    // -----------------------------------------------------------------------

    #[allow(dead_code)]
    pub async fn get_server_config(&self) -> Option<ServerConfig> {
        self.inner.read().await.server_config.clone()
    }

    pub async fn set_server_config(&self, cfg: ServerConfig) {
        self.inner.write().await.server_config = Some(cfg);
    }

    #[allow(dead_code)]
    pub async fn get_world_config(&self) -> Option<WorldConfig> {
        self.inner.read().await.world_config.clone()
    }

    pub async fn set_world_config(&self, cfg: WorldConfig) {
        self.inner.write().await.world_config = Some(cfg);
    }

    // -----------------------------------------------------------------------
    // Log ring buffer
    // -----------------------------------------------------------------------

    /// Append a log line to the ring buffer. Evicts the oldest entry when at
    /// capacity and broadcasts the event to connected WS clients.
    pub async fn push_log_line(&self, line: LogLine) {
        let mut inner = self.inner.write().await;
        if inner.log_buffer.len() == inner.log_capacity {
            inner.log_buffer.pop_front();
        }
        inner.log_buffer.push_back(line.clone());
        drop(inner);
        self.event_hub.publish(WsEvent::LogLine(line));
    }

    /// Return a snapshot of the current log buffer (oldest first).
    pub async fn get_log_snapshot(&self) -> Vec<LogLine> {
        self.inner
            .read()
            .await
            .log_buffer
            .iter()
            .cloned()
            .collect()
    }

    // -----------------------------------------------------------------------
    // Player state
    // -----------------------------------------------------------------------

    /// Record a player joining: add to online map and append to event buffer.
    pub async fn player_joined(&self, name: &str) {
        let now = Utc::now();
        let player = Player {
            name: name.to_string(),
            joined_at: now,
        };
        let event = PlayerEvent {
            player_name: name.to_string(),
            kind: PlayerEventKind::Joined,
            timestamp: now,
        };

        let mut inner = self.inner.write().await;
        inner.players.insert(name.to_string(), player);
        if inner.player_events.len() == inner.player_event_capacity {
            inner.player_events.pop_front();
        }
        inner.player_events.push_back(event);
        drop(inner);

        self.event_hub.publish(WsEvent::PlayerJoined {
            player_name: name.to_string(),
        });

        self.persist_history().await;
    }

    /// Record a player leaving: remove from online map and append to event buffer.
    pub async fn player_left(&self, name: &str) {
        let now = Utc::now();
        let event = PlayerEvent {
            player_name: name.to_string(),
            kind: PlayerEventKind::Left,
            timestamp: now,
        };

        let mut inner = self.inner.write().await;
        inner.players.remove(name);
        if inner.player_events.len() == inner.player_event_capacity {
            inner.player_events.pop_front();
        }
        inner.player_events.push_back(event);
        drop(inner);

        self.event_hub.publish(WsEvent::PlayerLeft {
            player_name: name.to_string(),
        });

        self.persist_history().await;
    }

    /// Return the list of currently-online players.
    pub async fn get_players(&self) -> Vec<Player> {
        self.inner
            .read()
            .await
            .players
            .values()
            .cloned()
            .collect()
    }

    /// Return the recent player-event history (oldest first).
    pub async fn get_player_events(&self) -> Vec<PlayerEvent> {
        self.inner
            .read()
            .await
            .player_events
            .iter()
            .cloned()
            .collect()
    }

    /// Clear the online player list (used when the server stops).
    pub async fn clear_players(&self) {
        self.inner.write().await.players.clear();
    }

    // -----------------------------------------------------------------------
    // Full state snapshot
    // -----------------------------------------------------------------------

    pub async fn snapshot(&self) -> AppStateSnapshot {
        let inner = self.inner.read().await;
        let player_count = inner.players.len();
        let players: Vec<Player> = inner.players.values().cloned().collect();
        AppStateSnapshot {
            server: inner.server_info.clone(),
            server_config: inner.server_config.clone(),
            world_config: inner.world_config.clone(),
            recent_logs: inner.log_buffer.iter().cloned().collect(),
            players,
            player_count,
            player_events: inner.player_events.iter().cloned().collect(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            snapshot_at: Utc::now(),
            backup: inner.backup_status.clone(),
            schedule: inner.schedule_state.clone(),
            install: inner.install_state.clone(),
            update: inner.update_state.clone(),
        }
    }

    // -----------------------------------------------------------------------
    // Backup state
    // -----------------------------------------------------------------------

    pub async fn get_backup_status(&self) -> BackupStatus {
        self.inner.read().await.backup_status.clone()
    }

    pub async fn set_backup_running(&self, progress_pct: Option<u8>, current_file: Option<String>) {
        let mut inner = self.inner.write().await;
        inner.backup_status.job_state = BackupJobState::Running;
        inner.backup_status.progress_pct = progress_pct;
        inner.backup_status.current_file = current_file;
        inner.backup_status.last_error = None;
    }

    pub async fn finish_backup(&self, entry: BackupEntry) {
        let mut inner = self.inner.write().await;
        inner.backup_status.job_state = BackupJobState::Done;
        inner.backup_status.progress_pct = Some(100);
        inner.backup_status.current_file = None;
        let entry_clone = entry.clone();
        inner.backup_status.history.push(entry);
        drop(inner);
        self.event_hub.publish(WsEvent::BackupProgress {
            job_state: "done".to_string(),
            progress_pct: Some(100),
            current_file: None,
            entry: Some(entry_clone),
        });
    }

    pub async fn set_backup_error(&self, error: String) {
        let mut inner = self.inner.write().await;
        inner.backup_status.job_state = BackupJobState::Failed;
        inner.backup_status.progress_pct = None;
        inner.backup_status.current_file = None;
        inner.backup_status.last_error = Some(error.clone());
        drop(inner);
        self.event_hub.publish(WsEvent::BackupProgress {
            job_state: "failed".to_string(),
            progress_pct: None,
            current_file: None,
            entry: None,
        });
    }

    // -----------------------------------------------------------------------
    // Schedule state
    // -----------------------------------------------------------------------

    pub async fn get_schedule_state(&self) -> ScheduleState {
        self.inner.read().await.schedule_state.clone()
    }

    pub async fn set_schedule_config(&self, config: crate::models::ScheduleConfig) {
        self.inner.write().await.schedule_state.config = config;
    }

    pub async fn set_countdown_active(&self, active: bool, seconds_remaining: Option<u64>) {
        let mut inner = self.inner.write().await;
        inner.schedule_state.countdown_active = active;
        inner.schedule_state.countdown_seconds_remaining = seconds_remaining;
    }

    pub async fn set_last_restart_date(&self, date: Option<String>) {
        self.inner.write().await.schedule_state.last_restart_date = date;
    }

    // -----------------------------------------------------------------------
    // Install state
    // -----------------------------------------------------------------------

    pub async fn get_install_state(&self) -> InstallState {
        self.inner.read().await.install_state.clone()
    }

    pub async fn set_install_state(
        &self,
        job_state: InstallJobState,
        progress_pct: Option<u8>,
        current_file: Option<String>,
        destination: Option<String>,
    ) {
        let mut inner = self.inner.write().await;
        inner.install_state.job_state = job_state;
        inner.install_state.progress_pct = progress_pct;
        inner.install_state.current_file = current_file;
        if let Some(dest) = destination {
            inner.install_state.destination = Some(dest);
        }
        inner.install_state.last_error = None;
    }

    pub async fn set_install_detected(&self, sources: Vec<String>) {
        let mut inner = self.inner.write().await;
        inner.install_state.job_state = InstallJobState::Detected;
        inner.install_state.detected_sources = sources;
        inner.install_state.last_error = None;
    }

    pub async fn set_install_error(&self, error: String) {
        let mut inner = self.inner.write().await;
        inner.install_state.job_state = InstallJobState::Failed;
        inner.install_state.last_error = Some(error);
    }

    // -----------------------------------------------------------------------
    // Update state
    // -----------------------------------------------------------------------

    pub async fn get_update_state(&self) -> UpdateState {
        self.inner.read().await.update_state.clone()
    }

    pub async fn set_update_checking(&self) {
        let mut inner = self.inner.write().await;
        inner.update_state.check_state = UpdateCheckState::Checking;
    }

    pub async fn set_update_result(
        &self,
        latest_version: String,
        update_available: bool,
        release_notes: Option<String>,
        download_url: Option<String>,
    ) {
        let mut inner = self.inner.write().await;
        inner.update_state.check_state = UpdateCheckState::Done;
        inner.update_state.latest_version = Some(latest_version.clone());
        inner.update_state.update_available = update_available;
        inner.update_state.last_checked_at = Some(Utc::now());
        inner.update_state.release_notes = release_notes;
        inner.update_state.download_url = download_url.clone();

        if update_available {
            let current = inner.update_state.current_version.clone();
            drop(inner);
            self.event_hub.publish(WsEvent::UpdateAvailable {
                current_version: current,
                latest_version,
                download_url,
            });
        }
    }

    pub async fn set_update_failed(&self, error: String) {
        let mut inner = self.inner.write().await;
        inner.update_state.check_state = UpdateCheckState::Failed;
        inner.update_state.last_checked_at = Some(Utc::now());
        drop(inner);
        tracing::warn!("Update check failed: {error}");
    }

    // -----------------------------------------------------------------------
    // History persistence
    // -----------------------------------------------------------------------

    /// Persist the player-event ring buffer to the configured history file.
    ///
    /// Runs as a detached tokio task so it does not block the caller.
    pub async fn persist_history(&self) {
        let path = match self.config.history_file_path.as_ref() {
            Some(p) => p.clone(),
            None => return,
        };
        let events = self.get_player_events().await;
        tokio::spawn(async move {
            match serde_json::to_string_pretty(&events) {
                Ok(json) => {
                    if let Err(e) = tokio::fs::write(&path, json).await {
                        tracing::warn!("Failed to persist player history to {}: {e}", path.display());
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to serialise player history: {e}");
                }
            }
        });
    }

    /// Load persisted player-event history from disk on startup.
    pub async fn load_history(&self) {
        let path = match self.config.history_file_path.as_ref() {
            Some(p) => p.clone(),
            None => return,
        };
        match tokio::fs::read_to_string(&path).await {
            Ok(json) => {
                match serde_json::from_str::<Vec<PlayerEvent>>(&json) {
                    Ok(events) => {
                        let mut inner = self.inner.write().await;
                        for event in events {
                            if inner.player_events.len() == inner.player_event_capacity {
                                inner.player_events.pop_front();
                            }
                            inner.player_events.push_back(event);
                        }
                        tracing::info!(
                            count = inner.player_events.len(),
                            "Loaded player event history from disk"
                        );
                    }
                    Err(e) => tracing::warn!("Failed to parse player history file: {e}"),
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // File doesn't exist yet; that's fine on first run.
            }
            Err(e) => tracing::warn!("Failed to read player history file: {e}"),
        }
    }
}

