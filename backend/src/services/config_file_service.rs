use std::path::{Path, PathBuf};
use std::time::SystemTime;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::config::AppConfig;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigFileKind {
    ServerDescription,
    WorldDescription,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigFileInfo {
    /// Path relative to the binary directory.
    pub path: String,
    pub file_name: String,
    pub kind: ConfigFileKind,
    pub last_modified: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigFileContent {
    pub path: String,
    pub content: String,
    pub last_modified: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct ConfigFileWrite {
    pub path: String,
    pub content: String,
    /// Client sends the last-known mtime; server checks for conflicts.
    pub last_modified: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct ValidateRequest {
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct ValidateResponse {
    pub valid: bool,
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// Discovery
// ---------------------------------------------------------------------------

/// Discover known config files (ServerDescription.json + WorldDescription.json)
/// within the server working directory.
pub fn discover_config_files(config: &AppConfig) -> Vec<ConfigFileInfo> {
    let binary_dir = match AppConfig::binary_dir() {
        Some(d) => d,
        None => return Vec::new(),
    };

    let working_dir = match config.server_working_dir.as_ref() {
        Some(d) => binary_dir.join(d),
        None => return Vec::new(),
    };

    let mut files = Vec::new();

    // ServerDescription.json lives in the server_working_dir root.
    let server_desc = working_dir.join("ServerDescription.json");
    if server_desc.is_file() {
        if let Some(info) = file_info(&binary_dir, &server_desc, ConfigFileKind::ServerDescription) {
            files.push(info);
        }
    }

    // WorldDescription.json files live under Saved/SaveProfiles/Default/RocksDB/**/Worlds/*/
    let worlds_base = working_dir
        .join("Saved")
        .join("SaveProfiles")
        .join("Default")
        .join("RocksDB");

    if worlds_base.is_dir() {
        scan_world_descriptions(&binary_dir, &worlds_base, &mut files);
    }

    files
}

fn scan_world_descriptions(binary_dir: &Path, rocks_dir: &Path, out: &mut Vec<ConfigFileInfo>) {
    // RocksDB/<version>/Worlds/<world_id>/WorldDescription.json
    let versions = match std::fs::read_dir(rocks_dir) {
        Ok(d) => d,
        Err(e) => {
            warn!("Cannot read RocksDB dir {}: {e}", rocks_dir.display());
            return;
        }
    };
    for version_entry in versions.flatten() {
        let worlds_dir = version_entry.path().join("Worlds");
        if !worlds_dir.is_dir() {
            continue;
        }
        let worlds = match std::fs::read_dir(&worlds_dir) {
            Ok(d) => d,
            Err(_) => continue,
        };
        for world_entry in worlds.flatten() {
            let desc = world_entry.path().join("WorldDescription.json");
            if desc.is_file() {
                if let Some(info) = file_info(binary_dir, &desc, ConfigFileKind::WorldDescription) {
                    out.push(info);
                }
            }
        }
    }
}

fn file_info(binary_dir: &Path, abs_path: &Path, kind: ConfigFileKind) -> Option<ConfigFileInfo> {
    let mtime = file_mtime(abs_path)?;
    let rel = abs_path
        .strip_prefix(binary_dir)
        .ok()?
        .to_string_lossy()
        .to_string();
    let file_name = abs_path
        .file_name()?
        .to_string_lossy()
        .to_string();
    Some(ConfigFileInfo {
        path: rel,
        file_name,
        kind,
        last_modified: mtime,
    })
}

// ---------------------------------------------------------------------------
// Read / Write / Validate
// ---------------------------------------------------------------------------

pub fn read_config_file(config: &AppConfig, rel_path: &str) -> Result<ConfigFileContent, String> {
    let abs = resolve_and_validate_path(config, rel_path)?;
    let content = std::fs::read_to_string(&abs)
        .map_err(|e| format!("Failed to read {}: {e}", abs.display()))?;
    let mtime = file_mtime(&abs)
        .ok_or_else(|| format!("Cannot read mtime for {}", abs.display()))?;
    Ok(ConfigFileContent {
        path: rel_path.to_string(),
        content,
        last_modified: mtime,
    })
}

pub fn write_config_file(
    config: &AppConfig,
    req: &ConfigFileWrite,
) -> Result<ConfigFileContent, WriteError> {
    // Validate JSON before touching disk.
    if let Err(e) = validate_json(&req.content) {
        return Err(WriteError::InvalidJson(e));
    }

    let abs = resolve_and_validate_path(config, &req.path)
        .map_err(WriteError::BadPath)?;

    // Conflict detection: compare mtime.
    let disk_mtime = file_mtime(&abs)
        .ok_or_else(|| WriteError::BadPath(format!("Cannot stat {}", abs.display())))?;

    let client_epoch = req.last_modified.timestamp_millis();
    let disk_epoch = disk_mtime.timestamp_millis();
    if (disk_epoch - client_epoch).abs() > 1000 {
        // Read current content so the client can diff.
        let disk_content = std::fs::read_to_string(&abs).unwrap_or_default();
        return Err(WriteError::Conflict {
            disk_content,
            disk_mtime,
        });
    }

    // Write.
    std::fs::write(&abs, &req.content)
        .map_err(|e| WriteError::Io(format!("Failed to write {}: {e}", abs.display())))?;

    let new_mtime = file_mtime(&abs)
        .ok_or_else(|| WriteError::Io("Cannot read mtime after write".to_string()))?;

    Ok(ConfigFileContent {
        path: req.path.clone(),
        content: req.content.clone(),
        last_modified: new_mtime,
    })
}

pub fn validate_json(content: &str) -> Result<serde_json::Value, String> {
    serde_json::from_str(content).map_err(|e| format!("Invalid JSON: {e}"))
}

pub fn get_file_mtime(config: &AppConfig, rel_path: &str) -> Result<DateTime<Utc>, String> {
    let abs = resolve_and_validate_path(config, rel_path)?;
    file_mtime(&abs).ok_or_else(|| format!("Cannot stat {}", abs.display()))
}

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

pub enum WriteError {
    InvalidJson(String),
    BadPath(String),
    Conflict {
        disk_content: String,
        disk_mtime: DateTime<Utc>,
    },
    Io(String),
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Resolve a relative path to an absolute path within the binary directory,
/// ensuring no path-traversal escapes.
fn resolve_and_validate_path(_config: &AppConfig, rel_path: &str) -> Result<PathBuf, String> {
    let binary_dir = AppConfig::binary_dir()
        .ok_or("Cannot determine binary directory")?;

    let abs = binary_dir.join(rel_path);
    let canonical = abs
        .canonicalize()
        .map_err(|e| format!("Path does not exist or is inaccessible: {e}"))?;
    let canonical_base = binary_dir
        .canonicalize()
        .map_err(|e| format!("Binary directory inaccessible: {e}"))?;

    if !canonical.starts_with(&canonical_base) {
        debug!(
            rel_path,
            canonical = %canonical.display(),
            base = %canonical_base.display(),
            "Path traversal blocked"
        );
        return Err("Path must be within the server directory".to_string());
    }

    Ok(canonical)
}

fn file_mtime(path: &Path) -> Option<DateTime<Utc>> {
    let meta = std::fs::metadata(path).ok()?;
    let st = meta.modified().ok()?;
    let dur = st.duration_since(SystemTime::UNIX_EPOCH).ok()?;
    Some(DateTime::from_timestamp(dur.as_secs() as i64, dur.subsec_nanos()).unwrap_or_default())
}
