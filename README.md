<div align="center">

# windrose-server-manager

  A self-contained server management tool for Windrose dedicated servers.  
  Single binary. Built-in browser dashboard. No external dependencies at runtime.

  <p>
    <a href="https://github.com/Rustbeard86/windrose-server-manager/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/Rustbeard86/windrose-server-manager/ci.yml?branch=main&style=for-the-badge" alt="Build Status"></a>
    <a href="https://github.com/Rustbeard86/windrose-server-manager/releases/latest"><img src="https://img.shields.io/github/v/release/Rustbeard86/windrose-server-manager?style=for-the-badge" alt="Latest Release"></a>
    <a href="https://github.com/Rustbeard86/windrose-server-manager/blob/main/CHANGELOG.md"><img src="https://img.shields.io/badge/changelog-here-informational?style=for-the-badge" alt="Changelog"></a>
  </p>

</div>

> [!NOTE]
> Place the binary next to `WindroseServer.exe`, run it, and open `http://127.0.0.1:8787`. The dashboard lets you start, stop, and monitor your Windrose server — no separate install, no extra config files, no external services.

---

## Table of Contents

- [What it does](#what-it-does)
- [Installation](#installation)
- [Usage](#usage)
- [Configuration](#configuration)
- [Troubleshooting](#troubleshooting)
- [Further reading](#further-reading)

---

## What it does

`windrose-server-manager` is a single-binary local web application that wraps a Windrose dedicated server process and exposes a browser dashboard for managing it.

- **Process control** — Start, stop, and restart the server. Detects crashes and transitions state automatically. The manager can be stopped or restarted independently — the game server process keeps running.
- **First-run wizard** — A guided setup wizard auto-detects the adjacent server binary, pre-fills all paths, and writes the config file. No manual JSON editing required on a standard install. Accessible any time from the header.
- **Live log feed** — Tails the server log file in real time and streams lines to the browser over WebSocket.
- **Player tracking** — Detects join/leave events from log output. Shows an online player list and session history.
- **Resource monitoring** — Live process CPU %, RAM usage, server-folder disk size, and network throughput. Displayed as arc gauges on the Dashboard while the server is running.
- **Backups** — Creates timestamped directory backups of the server working directory, with live progress.
- **Scheduled restarts** — Configure a daily restart at a specific time with a configurable countdown warning.
- **Config editing** — Read and write `ServerDescription.json` and `WorldDescription.json` from the dashboard. Structured field view, raw JSON editor, external-edit detection, and conflict diff.
- **Install / detect** — Probes common Steam library paths to locate a Windrose server source and copies it to a destination.
- **Self-update** — Checks GitHub Releases for a newer manager version and applies it in one click while leaving the game server untouched.

The entire React/TypeScript frontend is compiled into the binary at build time via `rust-embed`. No `static/` directory or separate web server is needed at runtime.

---

## Installation

### Download a release

Download the latest `windrose-server-manager.exe` from the [Releases](https://github.com/Rustbeard86/windrose-server-manager/releases/latest) page and place it in the same directory as `WindroseServer.exe`.

### Build from source

```bash
git clone https://github.com/Rustbeard86/windrose-server-manager.git
cd windrose-server-manager/backend
cargo build --release
```

See [docs/development.md](docs/development.md) for full build prerequisites and the development workflow.

---

## Usage

Place the binary next to `WindroseServer.exe` and run it:

```bat
windrose-server-manager.exe
```

The banner is printed to the console on startup:

```
╔══════════════════════════════════════════════════╗
║   Windrose Server Manager v0.1.0                 ║
╠══════════════════════════════════════════════════╣
║  Listening on http://127.0.0.1:8787              ║
║  API:       http://127.0.0.1:8787/api/health     ║
║  WebSocket: ws://127.0.0.1:8787/ws               ║
║  Press Ctrl+C to stop (server keeps running)     ║
╚══════════════════════════════════════════════════╝
```

Open `http://127.0.0.1:8787` in your browser to access the dashboard.

Pressing **Ctrl+C** stops the manager process. The game server process keeps running and will be re-adopted automatically the next time the manager starts.

### Environment variables

| Variable | Description |
|----------|-------------|
| `RUST_LOG` | Log verbosity. Defaults to `info`. Set to `debug` for verbose output. |

---

## Configuration

On first run, `windrose-server-manager.exe` writes a `windrose-server-manager.json` template in the same directory as the binary. The defaults are pre-configured for the standard Windrose server layout:

```json
{
  "bind_address": "127.0.0.1",
  "port": 8787,
  "server_executable": "WindroseServer.exe",
  "server_args": [],
  "server_working_dir": "R5",
  "log_file_path": "R5\\Saved\\Logs\\R5.log",
  "log_buffer_capacity": 500,
  "server_stop_timeout_secs": 15,
  "player_event_capacity": 200,
  "backup_dir": "backups",
  "history_file_path": null,
  "update_check_url": "https://api.github.com/repos/Rustbeard86/windrose-server-manager/releases/latest"
}
```

Paths can be relative (resolved from the directory containing the binary) or absolute. Any field omitted from the file falls back to the compiled-in default.

| Field | Default | Description |
|-------|---------|-------------|
| `bind_address` | `"127.0.0.1"` | Interface to listen on. Set to `"0.0.0.0"` to accept connections from other machines (e.g. accessing the dashboard via Steam overlay from a client while the server runs on a dedicated host) |
| `port` | `8787` | HTTP port |
| `server_executable` | `"WindroseServer.exe"` | Path to the server executable |
| `server_args` | `[]` | Extra arguments forwarded to the server on start |
| `server_working_dir` | `"R5"` | Server working directory — `ServerDescription.json` and `WorldDescription.json` are resolved here |
| `log_file_path` | `"R5\Saved\Logs\R5.log"` | Path to the log file to tail |
| `log_buffer_capacity` | `500` | Maximum log lines held in memory |
| `server_stop_timeout_secs` | `15` | Seconds to wait for graceful stop before force-kill |
| `player_event_capacity` | `200` | Maximum player events retained |
| `backup_dir` | `"backups"` | Directory where backups are written |
| `history_file_path` | `null` | Path to persist player-event history across restarts. Leave `null` for in-memory only |
| `update_check_url` | GitHub Releases API | URL for update checks. Set to `""` to disable |

### Remote / dedicated-host setup

To manage a server on a dedicated host from the Steam overlay:

1. Set `"bind_address": "0.0.0.0"` so the manager accepts external connections.
2. Open port `8787` in your host's firewall or security group.
3. In the Steam overlay browser, navigate to `http://<server-ip>:8787`.

> [!IMPORTANT]
> Binding to `0.0.0.0` exposes the management dashboard to the network. Only do this on hosts you control, and consider restricting access at the firewall level to trusted IPs.

---

## Troubleshooting

**Dashboard shows a blank page or 404** — the frontend bundle was not embedded. Run `cargo build --release` from `backend/` to trigger a full frontend rebuild and re-embed.

**Server won't start** — check that `server_executable` in `windrose-server-manager.json` points to your server `.exe`. If the file doesn't exist yet, run the manager once to generate it, then edit it.

**Log tailing shows no output** — verify that `log_file_path` is correct and that the server has been started at least once (the log file is created by the server on first run). The manager waits up to 30 seconds for the file to appear.

**Commands sent to the server have no effect** — many Windows game servers read from the Windows console input buffer rather than their stdin pipe. Commands are forwarded to the pipe on a best-effort basis.

**Steam source not detected** — the detect step probes a fixed list of common Steam library paths. If your library is in a non-standard location, pass the source path directly to the install panel.

**Manager update fails** — if `POST /api/update/apply` leaves a `windrose-server-manager-new.exe` behind, the download completed but the updater script could not run. Manually: stop the manager, rename the `-new.exe` to `windrose-server-manager.exe`, and restart.

---

## Further reading

- [API Reference & WebSocket Events](docs/api.md)
- [Development Guide](docs/development.md)
- [Changelog](CHANGELOG.md)
