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
//! All matching is case-insensitive via `to_lowercase()`.  No regex crate is
//! needed — simple `str::contains` + word extraction covers all patterns.

use tracing::debug;

use crate::state::AppState;

// ---------------------------------------------------------------------------
// Name extraction
// ---------------------------------------------------------------------------

/// Find `keyword` (ASCII, already lowercased) inside `lowered`, then extract
/// the first whitespace-delimited token that follows from the *original* line,
/// stripping any surrounding single-quotes.
///
/// Because `keyword` is pure ASCII, `keyword.len()` is the same byte offset in
/// both `lowered` and `original`, so it is safe to apply the offset to both.
fn extract_name_after(original: &str, lowered: &str, keyword: &str) -> Option<String> {
    let offset = lowered.find(keyword)? + keyword.len();
    let rest = original.get(offset..)?.trim_start();
    let rest = rest.strip_prefix('\'').unwrap_or(rest);
    let name: String = rest
        .chars()
        .take_while(|&c| c != '\'' && !c.is_ascii_whitespace())
        .collect();
    if name.is_empty() { None } else { Some(name) }
}

/// Check `lowered` for any of the supplied ASCII keywords and return the first
/// match, or `None`.
macro_rules! first_match {
    ($lower:expr, $orig:expr, [$($kw:expr),+ $(,)?]) => {{
        None $(.or_else(|| extract_name_after($orig, $lower, $kw)))*
    }};
}

pub(crate) fn detect_join(line: &str) -> Option<String> {
    let lower = line.to_lowercase();
    if lower.contains("has joined") {
        first_match!(&lower, line, ["player '", "player "])
    } else if lower.contains("connected") {
        first_match!(&lower, line, ["player '", "player ", "client "])
    } else if lower.contains("[join]") {
        extract_name_after(line, &lower, "[join]")
    } else {
        None
    }
}

pub(crate) fn detect_leave(line: &str) -> Option<String> {
    let lower = line.to_lowercase();
    if lower.contains("has left") {
        first_match!(&lower, line, ["player '", "player "])
    } else if lower.contains("disconnected") {
        first_match!(&lower, line, ["player '", "player ", "client "])
    } else if lower.contains("[leave]") {
        extract_name_after(line, &lower, "[leave]")
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Inspect a single log line for player join/leave events and update state.
///
/// Called by `log_service::ingest_raw` for every line tailed from the server
/// log file, so it must be cheap.
pub async fn process_line(state: &AppState, line: &str) {
    if let Some(player) = detect_join(line) {
        debug!(player, "Player joined (detected from log)");
        state.player_joined(&player).await;
        return;
    }
    if let Some(player) = detect_leave(line) {
        debug!(player, "Player left (detected from log)");
        state.player_left(&player).await;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_join_has_joined() {
        assert_eq!(detect_join("Player Rustbeard86 has joined"), Some("Rustbeard86".into()));
    }

    #[test]
    fn detects_join_connected_quoted() {
        assert_eq!(detect_join("[INFO] Player 'SomeUser' connected"), Some("SomeUser".into()));
    }

    #[test]
    fn detects_join_client() {
        assert_eq!(detect_join("Client Hero123 connected from 192.168.1.1"), Some("Hero123".into()));
    }

    #[test]
    fn detects_leave_has_left() {
        assert_eq!(detect_leave("Player Rustbeard86 has left"), Some("Rustbeard86".into()));
    }

    #[test]
    fn detects_leave_disconnected() {
        assert_eq!(detect_leave("[INFO] Player 'SomeUser' disconnected"), Some("SomeUser".into()));
    }

    #[test]
    fn detects_leave_client() {
        assert_eq!(detect_leave("Client Hero123 disconnected"), Some("Hero123".into()));
    }

    #[test]
    fn no_match_on_unrelated_line() {
        assert_eq!(detect_join("Server started on port 7777"), None);
        assert_eq!(detect_leave("Server started on port 7777"), None);
    }
}
