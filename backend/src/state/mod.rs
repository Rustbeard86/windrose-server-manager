use chrono::Utc;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex, RwLock};

use crate::config::AppConfig;
use crate::models::{
    AppStateSnapshot, LogLine, Player, PlayerEvent, PlayerEventKind, ServerConfig, ServerInfo,
    WsEvent, WorldConfig,
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
            })),
            event_hub: EventHub::new(),
            config: Arc::new(config),
            process: Arc::new(Mutex::new(None)),
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
        }
    }
}

