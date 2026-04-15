use std::path::Path;
use tracing::warn;

use crate::models::{ServerConfig, WorldConfig};
use crate::state::AppState;

/// Read server configuration from disk and cache it in application state.
///
/// Future implementation will parse `ServerDescription.json` (or equivalent)
/// from the configured server working directory and map it into `ServerConfig`.
pub async fn load_server_config(state: &AppState) -> Result<ServerConfig, String> {
    if let Some(path) = state.config.server_working_dir.as_ref() {
        let cfg_path = path.join("ServerDescription.json");
        if cfg_path.exists() {
            return read_server_config_from_file(&cfg_path).await;
        }
    }

    warn!("No server working directory configured; returning default ServerConfig scaffold");
    let default_cfg = ServerConfig {
        server_name: "Windrose Server".to_string(),
        max_players: 10,
        port: 7777,
        invite_code: None,
        extra: serde_json::Value::Object(serde_json::Map::new()),
    };
    state.set_server_config(default_cfg.clone()).await;
    Ok(default_cfg)
}

/// Read world configuration from disk and cache it in application state.
///
/// Future implementation will locate and parse `WorldDescription.json`.
pub async fn load_world_config(state: &AppState) -> Result<WorldConfig, String> {
    if let Some(path) = state.config.server_working_dir.as_ref() {
        let cfg_path = path.join("WorldDescription.json");
        if cfg_path.exists() {
            return read_world_config_from_file(&cfg_path).await;
        }
    }

    warn!("No world config found; returning default WorldConfig scaffold");
    let default_cfg = WorldConfig {
        world_name: "Windrose World".to_string(),
        seed: None,
        extra: serde_json::Value::Object(serde_json::Map::new()),
    };
    state.set_world_config(default_cfg.clone()).await;
    Ok(default_cfg)
}

/// Persist updated server configuration to disk.
///
/// Future implementation will write `ServerDescription.json` in a way that
/// preserves unrecognised fields (round-trip safe via `serde_json::Value`).
pub async fn save_server_config(state: &AppState, cfg: ServerConfig) -> Result<(), String> {
    if let Some(path) = state.config.server_working_dir.as_ref() {
        let cfg_path = path.join("ServerDescription.json");
        let json = serde_json::to_string_pretty(&cfg)
            .map_err(|e| format!("Serialisation error: {e}"))?;
        tokio::fs::write(&cfg_path, json)
            .await
            .map_err(|e| format!("Failed to write {}: {e}", cfg_path.display()))?;
    } else {
        warn!("No server working directory configured; config not persisted to disk");
    }
    state.set_server_config(cfg).await;
    Ok(())
}

/// Persist updated world configuration to disk.
pub async fn save_world_config(state: &AppState, cfg: WorldConfig) -> Result<(), String> {
    if let Some(path) = state.config.server_working_dir.as_ref() {
        let cfg_path = path.join("WorldDescription.json");
        let json = serde_json::to_string_pretty(&cfg)
            .map_err(|e| format!("Serialisation error: {e}"))?;
        tokio::fs::write(&cfg_path, json)
            .await
            .map_err(|e| format!("Failed to write {}: {e}", cfg_path.display()))?;
    } else {
        warn!("No server working directory configured; config not persisted to disk");
    }
    state.set_world_config(cfg).await;
    Ok(())
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

async fn read_server_config_from_file(path: &Path) -> Result<ServerConfig, String> {
    let raw = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
    serde_json::from_str(&raw).map_err(|e| format!("Failed to parse server config: {e}"))
}

async fn read_world_config_from_file(path: &Path) -> Result<WorldConfig, String> {
    let raw = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
    serde_json::from_str(&raw).map_err(|e| format!("Failed to parse world config: {e}"))
}
