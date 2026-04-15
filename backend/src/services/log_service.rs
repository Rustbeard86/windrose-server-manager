// Scaffold functions that will be wired up in future iterations are allowed
// to be unused at this stage.
#![allow(dead_code)]

use chrono::Utc;
use tracing::{info, warn};

use crate::models::{LogLevel, LogLine};
use crate::state::AppState;

/// Ingest a raw log string, parse it into a [`LogLine`], and push it into the
/// application state ring buffer.
///
/// This is the main hook for the future log-tailing integration. Currently it
/// accepts raw strings from any source (test harness, process stdout, etc.).
pub async fn ingest_raw(state: &AppState, raw: &str) {
    let line = parse_line(raw);
    info!(level = ?line.level, message = %line.message, "log ingested");
    state.push_log_line(line).await;
}

/// Parse a raw log string into a [`LogLine`].
///
/// Attempts simple heuristic detection of log levels. Future iterations can
/// implement full regex-based parsing tailored to the game server's format.
pub fn parse_line(raw: &str) -> LogLine {
    let level = detect_level(raw);
    LogLine {
        timestamp: Utc::now(),
        level,
        message: raw.trim().to_string(),
        raw: raw.to_string(),
    }
}

fn detect_level(raw: &str) -> LogLevel {
    let upper = raw.to_uppercase();
    if upper.contains("[ERROR]") || upper.contains("ERROR:") {
        LogLevel::Error
    } else if upper.contains("[WARN]") || upper.contains("WARNING:") || upper.contains("WARN:") {
        LogLevel::Warn
    } else if upper.contains("[DEBUG]") || upper.contains("DEBUG:") {
        LogLevel::Debug
    } else if upper.contains("[INFO]") || upper.contains("INFO:") {
        LogLevel::Info
    } else {
        LogLevel::Unknown
    }
}

/// Placeholder: begin tailing a log file at the given path.
///
/// In a future iteration this will:
/// 1. Open the file with shared read access (important on Windows).
/// 2. Seek to the end on first open.
/// 3. Continuously read new lines and call `ingest_raw`.
/// 4. Respect a cancellation signal so it can be shut down cleanly.
pub async fn start_log_tail(_state: AppState, log_path: std::path::PathBuf) {
    warn!(
        path = %log_path.display(),
        "log tailing not yet implemented — placeholder"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_error_level() {
        let line = parse_line("[ERROR] something bad happened");
        assert_eq!(line.level, LogLevel::Error);
    }

    #[test]
    fn parse_warn_level() {
        let line = parse_line("[WARN] disk space low");
        assert_eq!(line.level, LogLevel::Warn);
    }

    #[test]
    fn parse_unknown_level() {
        let line = parse_line("Server started on port 7777");
        assert_eq!(line.level, LogLevel::Unknown);
    }
}
