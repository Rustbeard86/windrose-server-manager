use std::path::{Path, PathBuf};
use tracing::warn;

use crate::models::{ServerConfig, WorldConfig};
use crate::state::AppState;

/// Read server configuration from disk and cache it in application state.
///
/// Future implementation will parse `ServerDescription.json` (or equivalent)
/// from the configured server working directory and map it into `ServerConfig`.
pub async fn load_server_config(state: &AppState) -> Result<ServerConfig, String> {
    if let Some(cfg_path) = resolved_server_config_path(state) {
        if cfg_path.exists() {
            let mut cfg = read_server_config_from_file(&cfg_path).await?;
            if cfg.port == 0 {
                cfg.port = state.config.port;
            }
            state.set_server_config(cfg.clone()).await;
            return Ok(cfg);
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
    if let Some(cfg_path) = resolved_world_config_path(state) {
        if cfg_path.exists() {
            let cfg = read_world_config_from_file(&cfg_path).await?;
            state.set_world_config(cfg.clone()).await;
            return Ok(cfg);
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
    if let Some(cfg_path) = resolved_server_config_path(state) {
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
    if let Some(cfg_path) = resolved_world_config_path(state) {
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
    let value: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| format!("Failed to parse server config: {e}"))?;

    if value.get("ServerDescription_Persistent").is_some() {
        let inner = value
            .get("ServerDescription_Persistent")
            .and_then(|v| v.as_object())
            .ok_or_else(|| "ServerDescription_Persistent must be an object".to_string())?;

        Ok(ServerConfig {
            server_name: inner
                .get("ServerName")
                .and_then(|v| v.as_str())
                .unwrap_or("Windrose Server")
                .to_string(),
            max_players: inner
                .get("MaxPlayerCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(10) as u32,
            port: value
                .get("Port")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u16,
            invite_code: inner
                .get("InviteCode")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(ToOwned::to_owned),
            extra: serde_json::Value::Object(serde_json::Map::new()),
        })
    } else {
        serde_json::from_value(value).map_err(|e| format!("Failed to parse server config: {e}"))
    }
}

async fn read_world_config_from_file(path: &Path) -> Result<WorldConfig, String> {
    let raw = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
    let value: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| format!("Failed to parse world config: {e}"))?;

    if value.get("WorldDescription").is_some() {
        let inner = value
            .get("WorldDescription")
            .and_then(|v| v.as_object())
            .ok_or_else(|| "WorldDescription must be an object".to_string())?;

        Ok(WorldConfig {
            world_name: inner
                .get("WorldName")
                .and_then(|v| v.as_str())
                .unwrap_or("Windrose World")
                .to_string(),
            seed: inner
                .get("islandId")
                .and_then(|v| v.as_str())
                .map(ToOwned::to_owned),
            extra: serde_json::Value::Object(serde_json::Map::new()),
        })
    } else {
        serde_json::from_value(value).map_err(|e| format!("Failed to parse world config: {e}"))
    }
}

fn resolved_working_dir(state: &AppState) -> Option<PathBuf> {
    let dir = state.config.server_working_dir.as_ref()?;
    if dir.is_absolute() {
        Some(dir.clone())
    } else {
        crate::config::AppConfig::binary_dir().map(|base| base.join(dir))
    }
}

pub fn resolved_server_config_path(state: &AppState) -> Option<PathBuf> {
    Some(resolved_working_dir(state)?.join("ServerDescription.json"))
}

pub fn resolved_world_config_path(state: &AppState) -> Option<PathBuf> {
    let working_dir = resolved_working_dir(state)?;
    let direct = working_dir.join("WorldDescription.json");
    if direct.exists() {
        return Some(direct);
    }

    let rocks_dir = working_dir.join("Saved").join("SaveProfiles").join("Default").join("RocksDB");
    find_latest_named_file(&rocks_dir, "WorldDescription.json").or(Some(direct))
}

fn find_latest_named_file(root: &Path, filename: &str) -> Option<PathBuf> {
    fn walk(dir: &Path, filename: &str, best: &mut Option<(std::time::SystemTime, PathBuf)>) {
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(_) => return,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, filename, best);
                continue;
            }
            if path.file_name().and_then(|n| n.to_str()) != Some(filename) {
                continue;
            }
            let modified = entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            let replace = match best {
                Some((current, _)) => modified > *current,
                None => true,
            };
            if replace {
                *best = Some((modified, path));
            }
        }
    }

    let mut best = None;
    walk(root, filename, &mut best);
    best.map(|(_, path)| path)
}
