//! Player join/leave detection from server log lines.
//!
//! # Log patterns
//!
//! The patterns below are derived from common Windrose dedicated-server log
//! output and are aligned with the detection logic in the original PowerShell
//! version of the manager.  They cover the most common variants observed:
//!
//! | Event  | Example line |
//! |--------|-------------|
//! | Join   | `Player Rustbeard86 has joined` |
//! | Join   | `[INFO] Player 'Rustbeard86' connected` |
//! | Join   | `Client Rustbeard86 connected from 127.0.0.1` |
//! | Leave  | `Player Rustbeard86 has left` |
//! | Leave  | `[INFO] Player 'Rustbeard86' disconnected` |
//! | Leave  | `Client Rustbeard86 disconnected` |
//!
//! All patterns are case-insensitive.  The player name is captured from the
//! first named group `name` in whichever pattern matches.
//!
//! If the server emits different patterns, additional entries can be added to
//! `JOIN_PATTERNS` / `LEAVE_PATTERNS` without touching the rest of the code.

use std::sync::OnceLock;

use regex::Regex;
use tracing::debug;

use crate::state::AppState;

// ---------------------------------------------------------------------------
// Pattern tables
// ---------------------------------------------------------------------------

/// Patterns that indicate a player has joined.  Each pattern must contain
/// a named capture group `(?P<name>...)`.
static JOIN_PATTERNS: &[&str] = &[
    r"(?i)player\s+'?(?P<name>[^'\s]+)'?\s+has\s+joined",
    r"(?i)player\s+'?(?P<name>[^'\s]+)'?\s+connected",
    r"(?i)client\s+(?P<name>\S+)\s+connected",
    r"(?i)\[join\]\s+(?P<name>\S+)",
];

/// Patterns that indicate a player has left.
static LEAVE_PATTERNS: &[&str] = &[
    r"(?i)player\s+'?(?P<name>[^'\s]+)'?\s+has\s+left",
    r"(?i)player\s+'?(?P<name>[^'\s]+)'?\s+disconnected",
    r"(?i)client\s+(?P<name>\S+)\s+disconnected",
    r"(?i)\[leave\]\s+(?P<name>\S+)",
];

// ---------------------------------------------------------------------------
// Compiled regex cache
// ---------------------------------------------------------------------------

struct CompiledPatterns {
    joins: Vec<Regex>,
    leaves: Vec<Regex>,
}

fn compiled() -> &'static CompiledPatterns {
    static CACHE: OnceLock<CompiledPatterns> = OnceLock::new();
    CACHE.get_or_init(|| CompiledPatterns {
        joins: JOIN_PATTERNS
            .iter()
            .filter_map(|p| Regex::new(p).ok())
            .collect(),
        leaves: LEAVE_PATTERNS
            .iter()
            .filter_map(|p| Regex::new(p).ok())
            .collect(),
    })
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Inspect a single log line for player join/leave events and update state.
///
/// Called by `log_service::ingest_raw` for every line tailed from the server
/// log file, so it must be cheap.  The regex patterns are compiled once and
/// reused.
pub async fn process_line(state: &AppState, line: &str) {
    let patterns = compiled();

    for re in &patterns.joins {
        if let Some(caps) = re.captures(line) {
            if let Some(name) = caps.name("name") {
                let player = name.as_str().to_string();
                debug!(player, "Player joined (detected from log)");
                state.player_joined(&player).await;
                return;
            }
        }
    }

    for re in &patterns.leaves {
        if let Some(caps) = re.captures(line) {
            if let Some(name) = caps.name("name") {
                let player = name.as_str().to_string();
                debug!(player, "Player left (detected from log)");
                state.player_left(&player).await;
                return;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn matches_join(line: &str) -> Option<String> {
        let patterns = compiled();
        for re in &patterns.joins {
            if let Some(caps) = re.captures(line) {
                if let Some(m) = caps.name("name") {
                    return Some(m.as_str().to_string());
                }
            }
        }
        None
    }

    fn matches_leave(line: &str) -> Option<String> {
        let patterns = compiled();
        for re in &patterns.leaves {
            if let Some(caps) = re.captures(line) {
                if let Some(m) = caps.name("name") {
                    return Some(m.as_str().to_string());
                }
            }
        }
        None
    }

    #[test]
    fn detects_join_has_joined() {
        assert_eq!(
            matches_join("Player Rustbeard86 has joined"),
            Some("Rustbeard86".into())
        );
    }

    #[test]
    fn detects_join_connected_quoted() {
        assert_eq!(
            matches_join("[INFO] Player 'SomeUser' connected"),
            Some("SomeUser".into())
        );
    }

    #[test]
    fn detects_join_client() {
        assert_eq!(
            matches_join("Client Hero123 connected from 192.168.1.1"),
            Some("Hero123".into())
        );
    }

    #[test]
    fn detects_leave_has_left() {
        assert_eq!(
            matches_leave("Player Rustbeard86 has left"),
            Some("Rustbeard86".into())
        );
    }

    #[test]
    fn detects_leave_disconnected() {
        assert_eq!(
            matches_leave("[INFO] Player 'SomeUser' disconnected"),
            Some("SomeUser".into())
        );
    }

    #[test]
    fn detects_leave_client() {
        assert_eq!(
            matches_leave("Client Hero123 disconnected"),
            Some("Hero123".into())
        );
    }

    #[test]
    fn no_match_on_unrelated_line() {
        assert_eq!(matches_join("Server started on port 7777"), None);
        assert_eq!(matches_leave("Server started on port 7777"), None);
    }
}
