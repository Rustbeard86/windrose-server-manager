use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Name of the JSON config file that lives adjacent to the binary.
const CONFIG_FILENAME: &str = "windrose-server-manager.json";

/// Top-level application configuration.
///
/// Loaded at startup from `windrose-server-manager.json` next to the binary.
/// If no file is present a template is written with defaults so the user has
/// a clear starting point.  Any fields omitted from the file fall back to the
/// compiled-in defaults, so a minimal config only needs to set the paths that
/// differ from defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Address to bind the HTTP server on.
    /// Use `"0.0.0.0"` to accept connections from other machines (e.g. a
    /// dedicated host that players connect to via the Steam overlay).
    pub bind_address: String,
    /// Port to listen on.
    pub port: u16,
    /// Path to the managed game-server executable.
    /// Example (Windows): `"C:\\WindroseServer\\WindroseServer.exe"`
    pub server_executable: Option<PathBuf>,
    /// Extra arguments forwarded to the server on start.
    pub server_args: Vec<String>,
    /// Server working directory — config and log files are resolved here.
    /// Example (Windows): `"C:\\WindroseServer"`
    pub server_working_dir: Option<PathBuf>,
    /// Path to the server log file to tail.
    /// The manager opens this with shared-read/write access so it can be read
    /// while the server holds it open for writing.
    /// Example (Windows): `"C:\\WindroseServer\\Saved\\Logs\\R5.log"`
    pub log_file_path: Option<PathBuf>,
    /// Maximum number of log lines held in memory.
    pub log_buffer_capacity: usize,
    /// Seconds to wait for a graceful stop before force-killing the process.
    pub server_stop_timeout_secs: u64,
    /// Maximum number of player join/leave events retained.
    pub player_event_capacity: usize,
    /// Directory where backup archives are written.
    pub backup_dir: PathBuf,
    /// Path for persisting player-event history across restarts.
    /// Leave `null` to keep history in memory only.
    pub history_file_path: Option<PathBuf>,
    /// GitHub Releases API URL for manager update checks.
    /// Set to `""` to disable update checks.
    pub update_check_url: String,

    // ── Schedule settings ──────────────────────────────────────────────────
    /// Whether the daily scheduled restart is enabled.
    #[serde(default)]
    pub schedule_enabled: bool,
    /// Hour (0–23) for the scheduled restart.
    #[serde(default = "default_schedule_hour")]
    pub schedule_restart_hour: u8,
    /// Minute (0–59) for the scheduled restart.
    #[serde(default)]
    pub schedule_restart_minute: u8,
    /// Warning countdown in seconds before the scheduled restart fires.
    #[serde(default = "default_warning_seconds")]
    pub schedule_warning_seconds: u64,
}

fn default_schedule_hour() -> u8 { 4 }
fn default_warning_seconds() -> u64 { 60 }

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            bind_address: "127.0.0.1".to_string(),
            port: 8787,
            server_executable: Some(PathBuf::from("WindroseServer.exe")),
            server_args: Vec::new(),
            server_working_dir: Some(PathBuf::from("R5")),
            log_file_path: Some(PathBuf::from(r"R5\Saved\Logs\R5.log")),
            log_buffer_capacity: 500,
            server_stop_timeout_secs: 15,
            player_event_capacity: 200,
            backup_dir: PathBuf::from("backups"),
            history_file_path: None,
            update_check_url: "https://api.github.com/repos/Rustbeard86/windrose-server-manager/releases/latest".to_string(),
            schedule_enabled: false,
            schedule_restart_hour: 4,
            schedule_restart_minute: 0,
            schedule_warning_seconds: 60,
        }
    }
}

impl AppConfig {
    /// Return the full socket address string, e.g. `"127.0.0.1:8787"`.
    pub fn socket_addr(&self) -> String {
        format!("{}:{}", self.bind_address, self.port)
    }

    /// The directory containing the running binary.
    pub fn binary_dir() -> Option<PathBuf> {
        std::env::current_exe().ok()?.parent().map(|p| p.to_path_buf())
    }

    /// Path to the config file: `<binary directory>/windrose-server-manager.json`.
    pub fn config_path() -> Option<PathBuf> {
        Some(Self::binary_dir()?.join(CONFIG_FILENAME))
    }

    /// Whether the configured server executable resolves to an existing file.
    pub fn server_executable_exists(&self) -> bool {
        let exe = match self.server_executable.as_ref() {
            Some(p) => p,
            None => return false,
        };
        if exe.is_absolute() {
            return exe.is_file();
        }
        match Self::binary_dir() {
            Some(base) => base.join(exe).is_file(),
            None => false,
        }
    }

    /// Persist the current config to the JSON file adjacent to the binary.
    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_path()
            .ok_or("Cannot determine config file path")?;
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Serialisation error: {e}"))?;
        std::fs::write(&path, &json)
            .map_err(|e| format!("Failed to write {}: {e}", path.display()))?;
        Ok(())
    }

    /// Load configuration from the JSON file adjacent to the binary.
    ///
    /// - **File absent**: writes a template with defaults so the user has a
    ///   clear starting point, then proceeds with defaults.
    /// - **File present, valid**: merges the file's values over the compiled-in
    ///   defaults, so a minimal config only needs the fields that differ.
    /// - **File present, invalid JSON**: prints an error and exits.
    pub fn load() -> Self {
        let defaults = Self::default();

        let path = match Self::config_path() {
            Some(p) => p,
            None => {
                tracing::warn!("Could not determine binary directory; using built-in defaults");
                return defaults;
            }
        };

        if !path.exists() {
            // Write a template so the user can see every available option.
            match serde_json::to_string_pretty(&defaults) {
                Ok(json) => match std::fs::write(&path, &json) {
                    Ok(_) => tracing::info!(
                        path = %path.display(),
                        "No config file found — wrote template with defaults. \
                         Edit it and restart to configure the server path."
                    ),
                    Err(e) => tracing::warn!(
                        path = %path.display(),
                        "Could not write config template: {e}"
                    ),
                },
                Err(e) => tracing::warn!("Could not serialise default config: {e}"),
            }
            return defaults;
        }

        // File exists — read it.
        let json = match std::fs::read_to_string(&path) {
            Ok(j) => j,
            Err(e) => {
                eprintln!(
                    "ERROR: Cannot read config file {}: {e}\nFix the file or delete it to regenerate defaults.",
                    path.display()
                );
                std::process::exit(1);
            }
        };

        // Parse as a raw JSON value so we can merge over defaults, allowing
        // the user to omit fields they are happy with.
        let partial: serde_json::Value = match serde_json::from_str(&json) {
            Ok(v) => v,
            Err(e) => {
                eprintln!(
                    "ERROR: Invalid JSON in {}: {e}\nFix the file or delete it to regenerate defaults.",
                    path.display()
                );
                std::process::exit(1);
            }
        };

        // Merge: start from defaults, overlay file values on top.
        let mut base = serde_json::to_value(&defaults)
            .expect("AppConfig::default() must be serialisable");
        if let (serde_json::Value::Object(ref mut base_map), serde_json::Value::Object(partial_map)) =
            (&mut base, partial)
        {
            for (k, v) in partial_map {
                base_map.insert(k, v);
            }
        }

        match serde_json::from_value::<Self>(base) {
            Ok(cfg) => {
                tracing::info!(path = %path.display(), "Loaded config");
                cfg
            }
            Err(e) => {
                eprintln!(
                    "ERROR: Config file {} has unrecognised structure: {e}\n\
                     Delete it to regenerate a fresh template.",
                    path.display()
                );
                std::process::exit(1);
            }
        }
    }
}
