use chrono::Utc;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

use crate::config::AppConfig;
use crate::models::{AppStateSnapshot, LogLine, ServerConfig, ServerInfo, WsEvent, WorldConfig};

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
// AppState — central shared state container
// ---------------------------------------------------------------------------

/// Inner mutable state, protected by an `RwLock`.
#[derive(Debug)]
struct Inner {
    server_info: ServerInfo,
    server_config: Option<ServerConfig>,
    world_config: Option<WorldConfig>,
    log_buffer: VecDeque<LogLine>,
    log_capacity: usize,
}

/// Shared application state accessible from all request handlers and services.
///
/// Clone cheaply — all clones share the same underlying data via `Arc`.
#[derive(Clone, Debug)]
pub struct AppState {
    inner: Arc<RwLock<Inner>>,
    pub event_hub: EventHub,
    pub config: Arc<AppConfig>,
}

impl AppState {
    pub fn new(config: AppConfig) -> Self {
        let capacity = config.log_buffer_capacity;
        Self {
            inner: Arc::new(RwLock::new(Inner {
                server_info: ServerInfo::default(),
                server_config: None,
                world_config: None,
                log_buffer: VecDeque::with_capacity(capacity),
                log_capacity: capacity,
            })),
            event_hub: EventHub::new(),
            config: Arc::new(config),
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
    #[allow(dead_code)]
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
    // Full state snapshot
    // -----------------------------------------------------------------------

    pub async fn snapshot(&self) -> AppStateSnapshot {
        let inner = self.inner.read().await;
        AppStateSnapshot {
            server: inner.server_info.clone(),
            server_config: inner.server_config.clone(),
            world_config: inner.world_config.clone(),
            recent_logs: inner.log_buffer.iter().cloned().collect(),
            player_count: 0,
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            snapshot_at: Utc::now(),
        }
    }
}
