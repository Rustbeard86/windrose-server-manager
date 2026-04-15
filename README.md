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
│       │   └── player_service.rs  – join/leave detection from log lines
│       └── api/
│           ├── mod.rs         – router builder
│           ├── health.rs      – GET  /api/health
│           ├── state.rs       – GET  /api/state
│           ├── server.rs      – POST /api/server/{start,stop,restart}
│           ├── command.rs     – POST /api/server/command
│           ├── config.rs      – GET/PUT /api/config/{server,world}
│           ├── logs.rs        – GET  /api/logs
│           ├── players.rs     – GET  /api/players
│           └── ws.rs          – WebSocket /ws
└── static/
    └── index.html     – placeholder Windrose-themed dashboard UI
```

---

## Prerequisites

- [Rust](https://rustup.rs/) (stable, 1.75+)
- Windows host recommended for full process-management functionality
- No other runtime dependencies — the binary is self-contained

---

## Running locally

```bash
# From the repository root
cd backend
cargo run
```

The backend starts on **http://127.0.0.1:8787** by default.

Open [http://127.0.0.1:8787](http://127.0.0.1:8787) in your browser to see
the placeholder dashboard.

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
| `static_dir` | `static` | Path to static frontend assets |
| `server_executable` | `None` | **Required** — path to the managed server `.exe` |
| `server_args` | `[]` | Extra CLI arguments forwarded to the server |
| `server_working_dir` | `None` | Server working directory (config/log files resolved here) |
| `log_file_path` | `None` | Path to the server log file to tail (`R5.log` typically) |
| `log_buffer_capacity` | `500` | Max log lines held in memory |
| `server_stop_timeout_secs` | `15` | Wait before force-killing on stop |
| `player_event_capacity` | `200` | Max player join/leave events retained |

A future iteration will load overrides from a JSON config file or environment
variables on startup.

---

## API reference

| Method | Path | Body | Description |
|--------|------|------|-------------|
| `GET` | `/api/health` | — | Liveness check — always `200 OK` |
| `GET` | `/api/state` | — | Full app state snapshot |
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
| `notification` | General notification (e.g. crash) |
| `ping` | Keepalive |

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

### Phase 2 — core control surface ✅ (this PR)
- [x] Real process spawn / kill / graceful stop via `tokio::process`
- [x] Background process watcher (crash detection)
- [x] stdin command forwarding (best-effort; Windows caveat documented)
- [x] Real incremental log tailing from file (Windows shared-access)
- [x] Player join/leave detection from log via regex
- [x] Online player list + event history in app state
- [x] Config JSON round-trip with unknown-field preservation
- [x] `POST /api/server/command` endpoint
- [x] `GET /api/players` endpoint

### Phase 3 — operational features
- [ ] Backup / restore service
- [ ] Scheduled restart with countdown events
- [ ] Install / update workflow
- [ ] Self-update mechanism
- [ ] Config file for AppConfig overrides (no recompile needed)

### Phase 4 — frontend
- [ ] React + Vite + Tailwind dashboard
- [ ] Live metrics (CPU / RAM / uptime)
- [ ] Player panel
- [ ] Console command input
- [ ] Config editor panels
- [ ] Windrose-themed visual design (deep navy / gold / teal)
