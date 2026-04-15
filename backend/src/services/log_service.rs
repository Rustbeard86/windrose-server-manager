//! Log ingestion, parsing, and real-time file-tailing.
//!
//! # File tailing on Windows
//!
//! On Windows, a process that owns a file for writing typically opens it with
//! `FILE_SHARE_READ` but *not* `FILE_SHARE_WRITE` — which is fine for readers.
//! We open the file with `FILE_SHARE_READ | FILE_SHARE_WRITE` (0x3) on Windows
//! so that even if the server opens it with `GENERIC_WRITE` we can still read.
//! On other platforms the standard `File::open` is used.
//!
//! The tail loop seeks to the end of the file on first open and then polls
//! every 250 ms for new bytes, keeping partial-line state across reads.

use std::path::PathBuf;
use std::time::Duration;

use chrono::Utc;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tracing::{info, warn};

use crate::models::{LogLevel, LogLine};
use crate::services::player_service;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Public ingest entry-point
// ---------------------------------------------------------------------------

/// Ingest a raw log string, parse it into a [`LogLine`], and push it into the
/// application state ring buffer.
pub async fn ingest_raw(state: &AppState, raw: &str) {
    let line = parse_line(raw);
    // Scan for player events before pushing to the ring buffer so that all
    // subscribers see player state updates alongside the log line.
    player_service::process_line(state, raw).await;
    state.push_log_line(line).await;
}

// ---------------------------------------------------------------------------
// Parse helpers
// ---------------------------------------------------------------------------

/// Parse a raw log string into a [`LogLine`].
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

// ---------------------------------------------------------------------------
// Real log-tailing task
// ---------------------------------------------------------------------------

/// Spawn a background task that incrementally reads new lines from `log_path`
/// and ingests them into `state`.
///
/// The task:
/// 1. Opens the file with shared-read/shared-write access (see module docs).
/// 2. Seeks to the end so we don't replay old log history on startup.
/// 3. Polls every 250 ms for new bytes.
/// 4. Accumulates partial lines across reads.
/// 5. Calls `ingest_raw` for each complete line.
///
/// The task runs until the process exits (it will never return an error; it
/// logs warnings internally).
pub fn start_log_tail(state: AppState, log_path: PathBuf) {
    tokio::spawn(async move {
        info!(path = %log_path.display(), "Starting log tail");

        // Retry loop: the log file may not exist yet if the server hasn't
        // started writing it.  We wait up to ~30 s before giving up.
        let mut retries = 0u32;
        let file = loop {
            match open_for_tail(&log_path) {
                Ok(f) => break f,
                Err(e) => {
                    // Log on first failure and then every ~10 retries to avoid
                    // spam while still giving visibility into the wait.
                    if retries == 0 || retries % 10 == 0 {
                        warn!(
                            path = %log_path.display(),
                            retries,
                            "Log file not yet available, waiting: {e}"
                        );
                    }
                    retries += 1;
                    if retries > 120 {
                        warn!(path = %log_path.display(), "Giving up waiting for log file after 30 s");
                        return;
                    }
                    tokio::time::sleep(Duration::from_millis(250)).await;
                }
            }
        };

        let mut file = tokio::fs::File::from_std(file);

        // Seek to the end so we only see new output.
        if let Err(e) = file
            .seek(std::io::SeekFrom::End(0))
            .await
        {
            warn!("Could not seek to end of log file: {e}");
        }

        let mut partial: String = String::new();
        let mut buf = vec![0u8; 8192];

        loop {
            match file.read(&mut buf).await {
                Ok(0) => {
                    // No new data; yield and retry.
                    tokio::time::sleep(Duration::from_millis(250)).await;
                }
                Ok(n) => {
                    let chunk = String::from_utf8_lossy(&buf[..n]);
                    partial.push_str(&chunk);

                    // Split on newlines, keeping any trailing incomplete line.
                    let mut lines: Vec<&str> = partial.lines().collect();
                    let has_trailing_newline =
                        partial.ends_with('\n') || partial.ends_with('\r');

                    let incomplete = if !has_trailing_newline {
                        lines.pop()
                    } else {
                        None
                    };

                    for line in lines {
                        ingest_raw(&state, line).await;
                    }

                    partial = incomplete.map(|s| s.to_string()).unwrap_or_default();
                }
                Err(e) => {
                    warn!("Log tail read error: {e}; retrying in 1 s");
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
    });
}

// ---------------------------------------------------------------------------
// Platform-specific file open for tailing
// ---------------------------------------------------------------------------

/// Open the log file in a way that allows reading while the server has it open
/// for writing.
///
/// On Windows we request `FILE_SHARE_READ | FILE_SHARE_WRITE` so that even if
/// the server holds an exclusive write handle we can still read.
/// On other platforms the default `File::open` is sufficient.
fn open_for_tail(path: &PathBuf) -> std::io::Result<std::fs::File> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // FILE_SHARE_READ  = 0x00000001
        // FILE_SHARE_WRITE = 0x00000002
        const FILE_SHARE_READ: u32 = 0x0000_0001;
        const FILE_SHARE_WRITE: u32 = 0x0000_0002;
        std::fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .open(path)
    }
    #[cfg(not(windows))]
    {
        std::fs::File::open(path)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

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

    #[test]
    fn parse_info_level() {
        let line = parse_line("[INFO] Player connected");
        assert_eq!(line.level, LogLevel::Info);
    }
}

