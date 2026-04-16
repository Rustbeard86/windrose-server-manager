# Development Guide

## Prerequisites

- [Rust](https://rustup.rs/) stable (1.75+)
- [Node.js](https://nodejs.org/) 18+ with npm

> Node.js is only needed at compile time to build the frontend bundle. The resulting binary has no Node.js dependency at runtime.

---

## Building

```bash
git clone https://github.com/Rustbeard86/windrose-server-manager.git
cd windrose-server-manager/backend
cargo build --release
```

The `build.rs` script runs `npm ci && npm run build` automatically. The compiled binary lands at `backend/target/release/windrose-server-manager.exe`.

### What the build does

1. `build.rs` runs `npm ci` in `frontend/` to restore exact dependencies
2. Runs `npm run build` (Vite) which outputs the compiled React app into `static/`
3. `rust-embed` bakes every file in `static/` into the binary at compile time

The `static/` directory is **not** read at runtime — it is only an intermediate build artefact.

---

## Development mode (hot-reload)

Run the Rust backend in one terminal:

```bash
cd backend
cargo run
```

In a second terminal, start the Vite dev server:

```bash
cd frontend
npm install      # first time only
npm run dev      # → http://localhost:5173
```

All `/api` and `/ws` requests from the Vite dev server are proxied to `127.0.0.1:8787`. The full app works with instant UI hot-reload without recompiling Rust.

### Verbose backend logging

```bash
# PowerShell
$env:RUST_LOG="debug"; cargo run

# cmd
set RUST_LOG=debug && cargo run
```

---

## Running tests

```bash
cd backend
cargo test
```

---

## Architecture

```
windrose-server-manager/
├── backend/                   # Rust binary (Axum 0.7, Tokio)
│   └── src/
│       ├── main.rs            – entry point: config load, PID re-adoption, graceful shutdown
│       ├── config/mod.rs      – AppConfig (JSON file + defaults)
│       ├── models/mod.rs      – strongly-typed request/response models
│       ├── pid.rs             – PID file: write/read/remove (server survives manager restart)
│       ├── process.rs         – ManagedProcess: spawn / kill / console command injection + stdin fallback
│       ├── state/mod.rs       – AppState shared container + EventHub (WS broadcast)
│       ├── services/
│       │   ├── server_service.rs       – start / stop / restart / send_command + PID tracking + stdout/stderr ingest
│       │   ├── config_service.rs       – read / write ServerDescription.json & WorldDescription.json
│       │   ├── config_file_service.rs  – raw config-file read/write with mtime conflict detection
│       │   ├── log_service.rs          – incremental log-tailing + unified line parsing/ingest
│       │   ├── player_service.rs       – join/leave detection from log lines
│       │   ├── backup_service.rs       – timestamped directory backups + progress events
│       │   ├── schedule_service.rs     – daily restart scheduler + countdown/cancel
│       │   ├── install_service.rs      – Steam source detection + install copy
│       │   ├── stats_service.rs        – background 2-s resource-stat collector (CPU, RAM, disk, network)
│       │   └── update_service.rs       – GitHub release check + self-update apply
│       └── api/
│           ├── mod.rs         – router builder
│           ├── health.rs      – GET  /api/health
│           ├── state.rs       – GET  /api/state
│           ├── server.rs      – POST /api/server/{start,stop,restart}
│           ├── command.rs     – POST /api/server/command
│           ├── stats.rs       – GET  /api/server/stats
│           ├── config.rs      – GET/PUT /api/config/{server,world} + config-file management
│           ├── setup.rs       – GET /api/setup/status, PUT /api/setup/config (FTUE wizard)
│           ├── auth.rs        – session auth, invites, reset codes, RBAC checks, CSRF middleware
│           ├── logs.rs        – GET  /api/logs
│           ├── players.rs     – GET  /api/players
│           ├── history.rs     – GET  /api/history/players
│           ├── backup.rs      – GET/POST /api/backup, POST /api/backup/create
│           ├── schedule.rs    – GET/PUT /api/schedule, POST /api/schedule/cancel
│           ├── install.rs     – GET/POST /api/install/{detect,run}
│           ├── update.rs      – GET /api/update, POST /api/update/{check,apply}
│           └── ws.rs          – WebSocket /ws
├── frontend/                  # React 18 + Vite + TypeScript SPA
│   ├── src/
│   │   ├── main.tsx           – React entry point
│   │   ├── App.tsx            – root shell (navigation, view routing, FTUE wizard gate)
│   │   ├── index.css          – design system tokens + global styles
│   │   ├── components/        – shared components (AppHeader, StatusBadge)
│   │   ├── hooks/             – useAppState (REST+WS hydration), useWebSocket
│   │   ├── types/api.ts       – TypeScript types matching backend models
│   │   ├── utils/             – api helpers, formatting
│   │   └── views/
│   │       ├── DashboardView.tsx    – server status, controls, stats gauges, activity feed
│   │       ├── LogsView.tsx         – live log feed
│   │       ├── PlayersView.tsx      – online players + event history
│   │       ├── ConfigView.tsx       – config file editor (structured + raw JSON tabs, diff view)
│   │       ├── OperationsView.tsx   – backup, schedule, update, install panels
│   │       └── SetupWizard.tsx      – first-run / re-run setup wizard
│   ├── vite.config.ts         – builds to ../static/; proxies /api and /ws in dev
│   └── package.json
└── static/
    └── ...                    # compiled frontend bundle (embedded into binary at build time)
```

### Self-update mechanism

When `POST /api/update/apply` is called:

1. The manager downloads the new binary to `windrose-server-manager-new.exe` (adjacent to the current binary)
2. An updater script (`windrose-manager-updater.bat`) is extracted and spawned as a detached process
3. The updater waits for the manager PID to exit, moves the new binary over the old one, restarts the manager, then deletes itself
4. The manager triggers its own shutdown via `GenerateConsoleCtrlEvent` (Windows) or `SIGINT` (Unix)

Leftover `*-new.exe` and `*-updater.bat` files from interrupted updates are cleaned up on every startup.

### Process ownership

The manager writes the server's PID to `windrose-server.pid` adjacent to the binary whenever the server starts. On manager restart:

- If the PID file exists and the process is still running, the manager re-adopts it (state transitions to `Running`) without touching the process
- The PID file is only removed when the server is explicitly stopped through the manager
- Pressing Ctrl+C stops the manager — the server process keeps running
