# Windrose Server Manager

A local backend binary for managing a dedicated Windrose game server, exposing
a small HTTP API and WebSocket endpoint suited for a browser-based dashboard UI.

---

## Architecture overview

```
windrose-server-manager/
├── backend/           # Rust binary (this crate)
│   └── src/
│       ├── main.rs            – entry point, server wiring, log-tail start
│       ├── config/mod.rs      – AppConfig (bind addr, paths, tuning)
│       ├── models/mod.rs      – strongly-typed request/response models
│       ├── process.rs         – ManagedProcess: spawn / kill / stdin
│       ├── state/mod.rs       – AppState shared container + EventHub (WS broadcast)
│       ├── services/
│       │   ├── server_service.rs  – start / stop / restart / send_command
│       │   ├── config_service.rs  – read / write server & world config JSON
│       │   ├── log_service.rs     – real incremental log-tailing + parse
│       │   ├── player_service.rs  – join/leave detection from log lines
│       │   ├── backup_service.rs  – create timestamped backups, progress events
│       │   ├── schedule_service.rs – daily restart scheduler + countdown/cancel
│       │   ├── install_service.rs  – Steam source detection + install copy
│       │   └── update_service.rs  – GitHub release check, update-state
│       └── api/
│           ├── mod.rs         – router builder
│           ├── health.rs      – GET  /api/health
│           ├── state.rs       – GET  /api/state
│           ├── server.rs      – POST /api/server/{start,stop,restart}
│           ├── command.rs     – POST /api/server/command
│           ├── config.rs      – GET/PUT /api/config/{server,world}
│           ├── logs.rs        – GET  /api/logs
│           ├── players.rs     – GET  /api/players
│           ├── history.rs     – GET  /api/history/players
│           ├── backup.rs      – GET/POST /api/backup, POST /api/backup/create
│           ├── schedule.rs    – GET/PUT /api/schedule, POST /api/schedule/cancel
│           ├── install.rs     – GET/POST /api/install/{detect,run}
│           ├── update.rs      – GET /api/update, POST /api/update/check
│           └── ws.rs          – WebSocket /ws
├── frontend/          # React + Vite + TypeScript SPA (Phase 4)
│   ├── src/
│   │   ├── main.tsx           – React entry point
│   │   ├── App.tsx            – root shell (navigation, view routing)
│   │   ├── index.css          – design system tokens + global styles
│   │   ├── components/        – shared components (AppHeader, StatusBadge)
│   │   ├── hooks/             – useAppState (REST+WS hydration), useWebSocket
│   │   ├── types/api.ts       – TypeScript types matching backend models
│   │   ├── utils/             – api helpers, formatting
│   │   └── views/             – per-feature panels (Dashboard, Logs, Players, Config, Operations)
│   ├── public/                – static assets (favicon, etc.)
│   ├── vite.config.ts         – builds to ../static/; proxies /api and /ws in dev
│   └── package.json
└── static/
    ├── index.html             – compiled React app (embedded into binary at build time)
    └── assets/                – hashed CSS + JS bundles (also embedded)
```

> **Single-binary model:** the `static/` directory is **not** read at runtime.
> All frontend files are compiled into the binary by `rust-embed` during
> `cargo build`.  The directory is only used as an intermediate build artefact.

---

## Prerequisites

- [Rust](https://rustup.rs/) (stable, 1.75+)
- [Node.js](https://nodejs.org/) 18+ and npm — **required at compile time** to build the frontend; not needed at runtime
- Windows host recommended for full process-management functionality
- No other runtime dependencies — the resulting binary is completely self-contained

---

## Building the single binary

```bash
# From the repository root — one command builds the entire chain:
cd backend
cargo build --release
```

When Cargo compiles the project the `build.rs` script automatically:
1. Runs `npm ci` in `frontend/` to restore exact dependencies
2. Runs `npm run build` (Vite) which outputs the compiled React app into `static/`
3. `rust-embed` then bakes every file in `static/` into the binary

The result is a single `.exe` / ELF binary in `backend/target/release/windrose-server-manager`
that serves the full dashboard with **no external files needed at runtime**.

```bash
# Run the release binary from anywhere — no static/ directory required:
./target/release/windrose-server-manager
```

The dashboard will be available at **http://127.0.0.1:8787**.

### Development mode (hot-reload UI + live Rust backend)

Run the Rust backend in one terminal (it re-embeds the last built frontend):

```bash
cd backend
cargo run
```

In a second terminal, start the Vite dev server with hot-reload:

```bash
cd frontend
npm install      # first time only
npm run dev      # → http://localhost:5173
```

All `/api` and `/ws` requests from the dev server are proxied to `127.0.0.1:8787`,
so the full app works with instant UI hot-reload without recompiling Rust.

### Logging verbosity

```bash
RUST_LOG=debug cargo run
```

---

## Configuration

`AppConfig` defaults (in `backend/src/config/mod.rs`):

| Field | Default | Description |
|-------|---------|-------------|
| `bind_address` | `127.0.0.1` | Interface to bind on (localhost only) |
| `port` | `8787` | HTTP port |
| `server_executable` | `None` | **Required** — path to the managed server `.exe` |
| `server_args` | `[]` | Extra CLI arguments forwarded to the server |
| `server_working_dir` | `None` | Server working directory (config/log files resolved here) |
| `log_file_path` | `None` | Path to the server log file to tail (`R5.log` typically) |
| `log_buffer_capacity` | `500` | Max log lines held in memory |
| `server_stop_timeout_secs` | `15` | Wait before force-killing on stop |
| `player_event_capacity` | `200` | Max player join/leave events retained |
| `backup_dir` | `./backups` | Directory where backup artifacts are stored |
| `history_file_path` | `None` | Optional path to persist player-event history across restarts |
| `update_check_url` | GitHub Releases API | URL for manager-app update checks |

A future iteration will load overrides from a JSON config file or environment
variables on startup.

---

## API reference

### Phase 1 & 2 endpoints

| Method | Path | Body | Description |
|--------|------|------|-------------|
| `GET` | `/api/health` | — | Liveness check — always `200 OK` |
| `GET` | `/api/state` | — | Full app state snapshot (includes Phase 3 fields) |
| `POST` | `/api/server/start` | — | Spawn the server process |
| `POST` | `/api/server/stop` | — | Graceful stop (force-kill fallback) |
| `POST` | `/api/server/restart` | — | Stop then start |
| `POST` | `/api/server/command` | `{"command":"..."}` | Send stdin command to server |
| `GET` | `/api/config/server` | — | Read server configuration |
| `PUT` | `/api/config/server` | `ServerConfig` JSON | Write server configuration |
| `GET` | `/api/config/world` | — | Read world configuration |
| `PUT` | `/api/config/world` | `WorldConfig` JSON | Write world configuration |
| `GET` | `/api/logs` | — | Recent log lines (ring buffer) |
| `GET` | `/api/players` | — | Online players + recent events |
| `GET` | `/ws` | — | WebSocket — live event stream |

### Phase 3 endpoints

| Method | Path | Body | Description |
|--------|------|------|-------------|
| `GET` | `/api/history/players` | — | Player join/leave event history (persisted if configured) |
| `GET` | `/api/backup` | — | Backup job state + history |
| `POST` | `/api/backup/create` | `{"label":"..."}` (optional) | Start a backup of `server_working_dir` |
| `GET` | `/api/schedule` | — | Scheduled-restart config + runtime state |
| `PUT` | `/api/schedule` | `ScheduleConfig` JSON | Update scheduled-restart config |
| `POST` | `/api/schedule/cancel` | — | Cancel in-progress countdown |
| `GET` | `/api/install` | — | Install / detect job state |
| `POST` | `/api/install/detect` | — | Probe Steam paths for Windrose source |
| `POST` | `/api/install/run` | `{"source":"...","destination":"..."}` | Copy server files |
| `GET` | `/api/update` | — | App update-check state |
| `POST` | `/api/update/check` | — | Trigger update check (GitHub releases) |

All REST endpoints return JSON:

```json
{ "success": true, "data": { ... }, "message": null }
```

### WebSocket events

Events are JSON text frames:

```json
{ "event": "<event_type>", "data": { ... } }
```

| Event | Trigger |
|-------|---------|
| `server_status_changed` | Server lifecycle transition |
| `log_line` | New log line ingested from file tail |
| `player_joined` | Player join detected from log |
| `player_left` | Player leave detected from log |
| `notification` | General notification (e.g. crash, restart) |
| `ping` | Keepalive |
| `backup_progress` | Backup job progress / completion |
| `schedule_countdown` | Restart countdown tick or cancellation |
| `install_progress` | Install / copy job progress |
| `update_available` | Newer manager version detected |

---

## Phase 2 capabilities

### Real process manager
- Spawns `WindroseServer.exe` (or any configured executable) via `tokio::process`
- Tracks PID and start time in app state
- Graceful stop: writes `stop\n` to the server's stdin pipe; waits up to
  `server_stop_timeout_secs`; falls back to `TerminateProcess` on Windows
- Crash detection: background watcher task transitions state to `crashed` on
  non-zero exit and broadcasts a `notification` event
- Restart: stop (with fallback) → start

### Command input
- `POST /api/server/command` forwards a text command to the server's stdin pipe
- **Windows stdin caveat**: many Windows game servers read from the Windows
  console input buffer rather than the Win32 stdin pipe handle.  Commands are
  written to the pipe (best-effort); whether the server acts on them depends on
  its own I/O implementation.  A future enhancement could use `WriteConsoleInput`
  to inject key events directly into the server's console buffer.

### Real log tailing
- Opens the log file with `FILE_SHARE_READ | FILE_SHARE_WRITE` on Windows
  (so reads succeed even while the server holds the file open for writing)
- Seeks to the end on first open (no replay of historical log)
- Polls every 250 ms for new bytes; handles partial lines across reads
- Waits up to 30 s for the file to appear (server may not create it immediately)
- Each new line is ingested into the 500-line ring buffer and broadcast over WS

### Player tracking
- Regex-based join/leave detection on every tailed log line
- Patterns cover common variants: `has joined`, `connected`, `has left`,
  `disconnected`, and bracket-prefixed forms
- Maintains a live online-player map in `AppState`
- Maintains a bounded 200-event history ring buffer
- `GET /api/players` returns both; `player_joined`/`player_left` WS events fire
  in real time
- Players are cleared from the online list when the server stops or crashes

### Config persistence
- `GET/PUT /api/config/server` reads and writes `ServerDescription.json` in the
  configured working directory
- `GET/PUT /api/config/world` reads and writes `WorldDescription.json`
- `#[serde(flatten)]` `extra` fields on both config structs preserve unknown
  JSON keys across read→write round-trips
- All typed fields carry `#[serde(default)]` so partial/variant files load cleanly

---

## Phase 3 capabilities

### Backup workflows
- `POST /api/backup/create` triggers a non-blocking background copy of
  `server_working_dir` to a timestamped subdirectory inside `backup_dir`
  (e.g. `backups/20240115_043000/`)
- Progress is broadcast as `backup_progress` WebSocket events (one per file)
- Completed backups are recorded in-memory with id, timestamp, path, and size
- `GET /api/backup` returns current job state and the history list
- **Limitation**: backup history is in-memory only and resets on restart.
  Persistence of the backup index is planned for a future phase.

#### Example backup creation

```bash
# No body required; supply optional label:
curl -X POST http://127.0.0.1:8787/api/backup/create \
  -H 'Content-Type: application/json' \
  -d '{"label":"before-update"}'

# Poll status:
curl http://127.0.0.1:8787/api/backup
```

### Scheduled restart
- `PUT /api/schedule` configures a daily restart at a specified hour/minute
  (local time) with a configurable warning countdown (default 60 s)
- A background task checks every 30 s and fires the countdown when the time
  window is reached; once per calendar day only
- The countdown broadcasts `schedule_countdown` events every second with
  `seconds_remaining` so a UI can show a live counter
- `POST /api/schedule/cancel` cancels an in-progress countdown; a
  `schedule_countdown` event with `cancelled: true` is broadcast

#### Example schedule configuration

```bash
# Enable daily restart at 04:00 with 120 s warning:
curl -X PUT http://127.0.0.1:8787/api/schedule \
  -H 'Content-Type: application/json' \
  -d '{"enabled":true,"restart_hour":4,"restart_minute":0,"warning_seconds":120}'

# Cancel the countdown:
curl -X POST http://127.0.0.1:8787/api/schedule/cancel
```

### Install / detect workflow
- `POST /api/install/detect` probes common Steam library paths for Windrose
  game directories and returns a list of candidates via `install_progress`
  events and `GET /api/install`
- `POST /api/install/run` copies server files from a specified source to a
  destination (both must be absolute paths); progress is broadcast per-file
- The copy is a recursive directory clone; no ZIP/archive format is used
- **Windows**: probes `C:\Program Files (x86)\Steam\steamapps\common`,
  `D:\SteamLibrary\steamapps\common`, and several other common roots
- **Non-Windows**: detection returns an empty list (paths don't exist);
  an explicit `source` path can still be passed to `run`

#### Example install workflow

```bash
# 1. Detect available sources:
curl -X POST http://127.0.0.1:8787/api/install/detect
curl http://127.0.0.1:8787/api/install   # poll for detected_sources

# 2. Run install to a configured destination:
curl -X POST http://127.0.0.1:8787/api/install/run \
  -H 'Content-Type: application/json' \
  -d '{"source":"C:\\Program Files (x86)\\Steam\\steamapps\\common\\WindroseServer","destination":"C:\\WindroseServer"}'
```

### App update groundwork
- `POST /api/update/check` queries the configured `update_check_url`
  (defaults to the GitHub Releases API for this repo) for the latest release tag
- Compares the current binary version against the retrieved tag (normalised for
  leading `v`); stores result in `UpdateState`
- If an update is available, broadcasts `update_available` WebSocket event with
  `current_version`, `latest_version`, and `download_url`
- `GET /api/update` returns the current check state at any time
- **Current limitations**:
  - Read-only: no download or self-update logic is implemented.  In-place
    binary replacement on Windows requires the running EXE to be replaced while
    locked by the OS; the recommended pattern (spawn a small updater process,
    exit the manager, updater replaces the file and restarts) is documented but
    not yet coded.
  - Network dependency: if the GitHub API is unreachable, the state transitions
    to `failed` gracefully.

### Player / history retention
- Player join/leave events are persisted to a JSON file whenever a new event
  fires (if `history_file_path` is set in `AppConfig`)
- On startup the persisted events are loaded back into the ring buffer so
  history survives manager restarts
- `GET /api/history/players` provides a dedicated endpoint for the event log,
  separate from the `GET /api/players` online-player snapshot
- Suitable for a future browser UI timeline / activity feed

---

## Windows assumptions and limitations

- The binary targets Windows as the primary deployment platform.  It compiles
  and runs on Linux/macOS too, but process management features are designed for
  the Windows Windrose server.
- Log file sharing uses Windows `FILE_SHARE_WRITE` semantics on Windows; on
  other platforms `File::open` is used.
- Process spawning uses `tokio::process::Command` with `stdin(Stdio::piped())`.
  If the server ignores the pipe (reads from console buffer instead), stdin
  commands will be silently buffered in the OS pipe.
- Ctrl+C handling / graceful daemon shutdown is not yet implemented.
- Steam source detection probes a hard-coded list of common Windows drive/path
  combinations; it does not yet parse the Windows registry or
  `libraryfolders.vdf` for custom Steam library locations.
- App self-update is detection-only in Phase 3 (see update groundwork above).

---

## Building a release binary

```bash
cd backend
cargo build --release
# Output: backend/target/release/windrose-server-manager[.exe]
```

---

## Running tests

```bash
cd backend
cargo test
```

---

## Roadmap

### Phase 1 — backend skeleton ✅
- [x] HTTP server bound to localhost
- [x] REST API scaffold
- [x] WebSocket broadcast scaffold
- [x] Shared app state + event hub
- [x] Log ring buffer
- [x] Service module stubs (server, config, log)
- [x] Static file serving with placeholder UI

### Phase 2 — core control surface ✅
- [x] Real process spawn / kill / graceful stop via `tokio::process`
- [x] Background process watcher (crash detection)
- [x] stdin command forwarding (best-effort; Windows caveat documented)
- [x] Real incremental log tailing from file (Windows shared-access)
- [x] Player join/leave detection from log via regex
- [x] Online player list + event history in app state
- [x] Config JSON round-trip with unknown-field preservation
- [x] `POST /api/server/command` endpoint
- [x] `GET /api/players` endpoint

### Phase 3 — operational tooling ✅ (this PR)
- [x] Backup creation (timestamped directory copy, progress events, history)
- [x] Scheduled restart (daily time, countdown, cancel, WS events)
- [x] Steam / source detection for Windows install paths
- [x] Install copy workflow (source → destination, per-file progress)
- [x] App update-check abstraction (GitHub Releases API, update-available event)
- [x] Player event history persistence (JSON file, load on startup)
- [x] `GET /api/history/players` endpoint
- [x] `GET/POST /api/backup` endpoints
- [x] `GET/PUT /api/schedule`, `POST /api/schedule/cancel` endpoints
- [x] `GET/POST /api/install/{detect,run}` endpoints
- [x] `GET /api/update`, `POST /api/update/check` endpoints
- [x] New WS event types: backup_progress, schedule_countdown, install_progress, update_available
- [x] README updated with Phase 3 documentation and examples

### Phase 4 — browser UI (this PR) ✅

- [x] React + Vite + TypeScript frontend scaffold in `frontend/`
- [x] Windrose-inspired design system: deep navy/gold/teal palette, CSS design tokens, global styles, component primitives
- [x] Navigation shell with animated compass-rose header and per-view routing
- [x] Dashboard view: hero region (server name, status, uptime, invite code, quick-action controls), live player list, recent activity feed, command console, server info card
- [x] Logs view: live scrolling log feed from WebSocket + `/api/logs`, level filter, text search, auto-scroll toggle
- [x] Players view: online players table with session timers, player join/leave event history feed
- [x] Config view: server config editor (name, port, max players, invite code) + world config editor (name, seed), with unsaved-changes tracking
- [x] Operations view: Backup panel (create with label, progress bar, history list), Schedule panel (enable/configure daily restart, countdown, cancel), Update panel (version check, download link), Install/Detect panel (Steam source detection, source→destination copy)
- [x] WebSocket integration: real-time state updates for server status, log lines, player events, backup progress, schedule countdown, install progress, update available
- [x] REST hydration on load and on WS reconnect, graceful reconnect loop (5 s)
- [x] Loading/error/reconnecting states
- [x] Motion: fade-in transitions on views/rows, pulse dots for live status, slow compass spin, progress bar animations
- [x] Build pipeline: `npm run build` outputs production bundle to `static/`; Vite dev server proxies `/api` and `/ws` to backend on port 8787
- [x] Backend serves compiled SPA via `ServeDir` (no backend changes required for Phase 4 integration)
- [x] README updated with Phase 4 architecture, development workflow, and build instructions
