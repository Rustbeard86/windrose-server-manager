# Windrose Server Manager

A local backend binary for managing a dedicated game server, exposing a small
HTTP API and WebSocket endpoint suited for a browser-based dashboard UI.

---

## Architecture overview

```
windrose-server-manager/
├── backend/           # Rust binary (this crate)
│   └── src/
│       ├── main.rs            – entry point, server wiring
│       ├── config/mod.rs      – AppConfig (bind addr, paths, tuning)
│       ├── models/mod.rs      – strongly-typed request/response models
│       ├── state/mod.rs       – AppState shared container + EventHub (WS broadcast)
│       ├── services/
│       │   ├── server_service.rs  – start / stop / restart scaffold
│       │   ├── config_service.rs  – read / write server & world config
│       │   └── log_service.rs     – ring-buffer log ingestion + parse scaffold
│       └── api/
│           ├── mod.rs         – router builder
│           ├── health.rs      – GET  /api/health
│           ├── state.rs       – GET  /api/state
│           ├── server.rs      – POST /api/server/{start,stop,restart}
│           ├── config.rs      – GET/PUT /api/config/{server,world}
│           ├── logs.rs        – GET  /api/logs
│           └── ws.rs          – WebSocket /ws
└── static/
    └── index.html     – placeholder Windrose-themed dashboard UI
```

---

## Prerequisites

- [Rust](https://rustup.rs/) (stable, 1.75+)
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

## API reference

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/health` | Liveness check — always `200 OK` while running |
| `GET` | `/api/state` | Full app state snapshot (server info, config, recent logs) |
| `POST` | `/api/server/start` | Start the managed server process |
| `POST` | `/api/server/stop` | Stop the managed server process |
| `POST` | `/api/server/restart` | Restart the managed server process |
| `GET` | `/api/config/server` | Read server configuration |
| `PUT` | `/api/config/server` | Write server configuration |
| `GET` | `/api/config/world` | Read world configuration |
| `PUT` | `/api/config/world` | Write world configuration |
| `GET` | `/api/logs` | Return recent log lines (ring buffer) |
| `GET` | `/ws` | WebSocket — live event stream |

All REST endpoints return JSON in the shape:

```json
{ "success": true, "data": { ... }, "message": null }
```

### WebSocket events

Events are JSON text frames with the shape:

```json
{ "event": "<event_type>", "data": { ... } }
```

| Event | Trigger |
|-------|---------|
| `server_status_changed` | Server lifecycle transition |
| `log_line` | New log line ingested |
| `player_joined` | Player join detected |
| `player_left` | Player leave detected |
| `notification` | General notification |
| `ping` | Keepalive |

---

## Configuration

`AppConfig` defaults (in `backend/src/config/mod.rs`):

| Field | Default | Description |
|-------|---------|-------------|
| `bind_address` | `127.0.0.1` | Interface to bind on (localhost only) |
| `port` | `8787` | HTTP port |
| `static_dir` | `static` | Path to static frontend assets |
| `server_executable` | `None` | Path to the managed server binary |
| `server_working_dir` | `None` | Server working directory |
| `log_buffer_capacity` | `500` | Max log lines held in memory |

A future iteration will load overrides from a JSON config file or environment
variables.

---

## Building a release binary

```bash
cd backend
cargo build --release
# Output: backend/target/release/windrose-server-manager
```

---

## Running tests

```bash
cd backend
cargo test
```

---

## Roadmap

### Phase 1 — backend skeleton ✅ (this PR)
- [x] HTTP server bound to localhost
- [x] REST API scaffold
- [x] WebSocket broadcast scaffold
- [x] Shared app state + event hub
- [x] Log ring buffer
- [x] Service module stubs (server, config, log)
- [x] Static file serving with placeholder UI

### Phase 2 — real process management
- [ ] Spawn / kill `WindroseServer.exe` via `tokio::process`
- [ ] Capture stdout/stderr → log service
- [ ] Send stdin commands
- [ ] Crash detection & auto-restart option

### Phase 3 — operational features
- [ ] Live log tailing from file
- [ ] Player join/leave detection from logs
- [ ] Backup / restore service
- [ ] Scheduled restart with countdown events
- [ ] Config file round-trip (preserve unknown fields)

### Phase 4 — frontend
- [ ] React + Vite + Tailwind dashboard
- [ ] Live metrics (CPU / RAM / uptime)
- [ ] Player panel
- [ ] Console command input
- [ ] Config editor panels
- [ ] Windrose-themed visual design
