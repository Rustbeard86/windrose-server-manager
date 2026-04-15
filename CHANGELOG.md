# Changelog

All notable changes to this project will be documented in this file.

---

## [0.1.0] - 2026-04-15

### Added
- **First-run setup wizard** — on first launch (or via the header "⚙ Setup" button) a guided wizard auto-detects the adjacent server binary, pre-fills all paths, and writes `windrose-server-manager.json`; no manual editing required on a standard install
- **Live resource-usage stats panel** on the Dashboard — process-specific CPU % and RAM, server-folder disk size, and system-wide network rx/tx; circular SVG arc gauges, colour-coded green → yellow → red; updates every 2 s while the server is running
- **Config editor overhaul** — file picker for `ServerDescription.json` / `WorldDescription.json`; structured tab with labelled fields; raw JSON tab with syntax-aware editing; filesystem polling for external edits with conflict detection and inline diff view
- Config file management API (`GET /api/config/files`, `GET /api/config/file`, `PUT /api/config/file`, `POST /api/config/file/validate`, `GET /api/config/file/mtime`)
- Setup / FTUE API (`GET /api/setup/status`, `PUT /api/setup/config`)
- `GET /api/server/stats` endpoint — returns latest collected `ServerStats`
- `stats_updated` WebSocket event pushed every collection cycle
- Local binary auto-detection (`detect_local_server()`) — probes the adjacent directory and `R5\Binaries\Win64\` for `WindroseServer.exe`; the setup wizard shows a green "detected" banner and pre-fills all path fields when found
- `-log` launch option checkbox in the setup wizard; persisted to `server_args` in config
- Schedule config (`enabled`, `restart_hour`, `restart_minute`, `warning_seconds`) persisted to `windrose-server-manager.json` on every `PUT /api/schedule`; restored on manager restart
- Install panel pre-fills Source from the first auto-detected source and Destination from the last configured install path
- PID file (`windrose-server.pid`) written on server start, read on manager restart to re-adopt a running server without interrupting it
- Pressing Ctrl+C stops the manager only — the game server process keeps running
- Self-update: `POST /api/update/apply` downloads the new binary, extracts an embedded updater script, spawns it detached, and shuts the manager down gracefully
- `UpdateApplyState` (Idle / Downloading / Applying / PendingRestart / Failed) exposed in `/api/update` and over WebSocket
- Operations view: Apply Update button with confirmation dialog and apply-state progress labels; release notes in collapsible block
- `POST /api/update/apply` endpoint (returns 409 if no update is pending, 202 on acceptance)
- Leftover `*-new.exe` and `*-updater.bat` artefacts from interrupted updates are cleaned up on startup
- Configuration loaded from `windrose-server-manager.json` adjacent to the binary; first run writes a template with sane defaults
- `bind_address` config field — set to `"0.0.0.0"` for remote / dedicated-host use via the Steam overlay
- Setting `update_check_url` to `""` disables update checks entirely
- Default paths pre-configured for the standard Windrose server layout (`WindroseServer.exe`, `R5/Saved/Logs/R5.log`)
- CI workflow (`.github/workflows/ci.yml`) — build and test on every push/PR to `main`
- Release workflow (`.github/workflows/release.yml`) — builds a versioned binary and attaches it to a GitHub Release on `v*` tags

### Changed
- **Binary size reduced from 8.3 MB to 3.2 MB** — `regex` crate removed (player detection rewritten with plain string matching); release profile uses `opt-level = "z"`, `lto = true`, `codegen-units = 1`, `strip = true`, `panic = "abort"`
- Server start resolves relative `server_executable` and `server_working_dir` paths via the binary directory instead of rejecting them
- Restart guard — `POST /api/server/restart` is rejected with `409` while the server is already Starting or Stopping to prevent overlapping lifecycle calls
- Install panel hides Source Path, Destination Path, and Run Install when a server executable is already configured; Detect and detected-sources list remain visible
- Download URL for self-update now uses the release asset's `browser_download_url` field (was incorrectly using `html_url`)
- "No update available" log message reduced to `INFO` level and shortened
- Manager no longer exits with an error when `update_check_url` is empty
- Config service looks for `ServerDescription.json` inside `server_working_dir` (typically `R5/`)
- dropdown/select options styled for dark theme (background and foreground explicit)

### Fixed
- False conflict detection in the config editor caused by millisecond-precision mtime parsing difference between Rust and JavaScript
- Scroll position reset to top on every silent config reload
- Nested `ServerDescription_Persistent` JSON structure round-tripped correctly through the structured editor
- Root path `/` returning 404 when the embedded asset service incorrectly classified it as a static asset request
- CLI banner padding misalignment
