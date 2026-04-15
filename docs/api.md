# API Reference

All endpoints return JSON. Successful responses use the envelope:

```json
{ "success": true, "data": { ... }, "message": null }
```

Error responses return a non-2xx status with `"success": false` and a `"message"` describing the problem.

---

## REST Endpoints

| Method | Path | Body | Description |
|--------|------|------|-------------|
| `GET` | `/api/health` | — | Liveness probe — always `200 OK` |
| `GET` | `/api/state` | — | Full app state snapshot (includes `server_configured`, `server_stats`) |
| `POST` | `/api/server/start` | — | Spawn the server process |
| `POST` | `/api/server/stop` | — | Graceful stop (force-kill fallback) |
| `POST` | `/api/server/restart` | — | Stop then start (rejected with `409` if already Starting/Stopping) |
| `POST` | `/api/server/command` | `{"command":"..."}` | Send a stdin command to the server |
| `GET` | `/api/server/stats` | — | Latest collected `ServerStats` (CPU, RAM, disk, network); `data` is `null` when server is not running |
| `GET` | `/api/logs` | — | Recent log lines from the ring buffer |
| `GET` | `/api/players` | — | Online players and recent events |
| `GET` | `/api/history/players` | — | Full player join/leave event history |
| `GET` | `/api/config/server` | — | Read `ServerDescription.json` |
| `PUT` | `/api/config/server` | `ServerConfig` JSON | Write `ServerDescription.json` |
| `GET` | `/api/config/world` | — | Read `WorldDescription.json` |
| `PUT` | `/api/config/world` | `WorldConfig` JSON | Write `WorldDescription.json` |
| `GET` | `/api/config/files` | — | List known config files with kind and last-modified |
| `GET` | `/api/config/file?path=...` | — | Read raw content + last-modified of a config file |
| `PUT` | `/api/config/file` | `{"path":"...","content":"...","last_modified":"..."}` | Write a config file; `last_modified` is used for optimistic conflict detection |
| `POST` | `/api/config/file/validate` | `{"content":"..."}` | Validate JSON content without writing |
| `GET` | `/api/config/file/mtime?path=...` | — | Return the current last-modified timestamp for a config file |
| `GET` | `/api/setup/status` | — | Setup wizard status: `needs_setup`, current config, and any auto-detected paths |
| `PUT` | `/api/setup/config` | `SetupApply` JSON | Apply setup form values and write `windrose-server-manager.json` |
| `GET` | `/api/backup` | — | Backup state and history |
| `POST` | `/api/backup/create` | `{"label":"..."}` | Start a backup (label optional) |
| `GET` | `/api/schedule` | — | Scheduled-restart config and state |
| `PUT` | `/api/schedule` | `ScheduleConfig` JSON | Update and persist the scheduled-restart config |
| `POST` | `/api/schedule/cancel` | — | Cancel an in-progress countdown |
| `GET` | `/api/install` | — | Install / detect job state |
| `POST` | `/api/install/detect` | — | Probe Steam paths for a Windrose server source |
| `POST` | `/api/install/run` | `{"source":"...","destination":"..."}` | Copy server files |
| `GET` | `/api/update` | — | Update-check state |
| `POST` | `/api/update/check` | — | Trigger an update check |
| `POST` | `/api/update/apply` | — | Download and apply a pending update |

---

## WebSocket

Connect to `ws://127.0.0.1:8787/ws`. Each message is a JSON text frame:

```json
{ "event": "<event_type>", "data": { ... } }
```

| Event | Trigger |
|-------|---------|
| `server_status_changed` | Server lifecycle transition (starting → running → stopped / crashed) |
| `log_line` | New line ingested from the log file |
| `player_joined` | Join event detected from log |
| `player_left` | Leave event detected from log |
| `backup_progress` | Backup job progress or completion |
| `schedule_countdown` | Restart countdown tick or cancellation |
| `install_progress` | Install / copy job progress |
| `update_available` | Newer manager version detected |
| `stats_updated` | Process/system resource stats refreshed (every ~2 s while server is running) |
| `notification` | General notification (crash, restart, etc.) |
| `ping` | Keepalive |

---

## Examples

### Start the server

```bash
curl -X POST http://127.0.0.1:8787/api/server/start
```

### Send a console command

```bash
curl -X POST http://127.0.0.1:8787/api/server/command \
  -H 'Content-Type: application/json' \
  -d '{"command":"status"}'
```

### Create a labelled backup

```bash
curl -X POST http://127.0.0.1:8787/api/backup/create \
  -H 'Content-Type: application/json' \
  -d '{"label":"before-update"}'
```

### Configure a daily restart at 04:00 with 2-minute warning

```bash
curl -X PUT http://127.0.0.1:8787/api/schedule \
  -H 'Content-Type: application/json' \
  -d '{"enabled":true,"restart_hour":4,"restart_minute":0,"warning_seconds":120}'
```

### Cancel a restart countdown

```bash
curl -X POST http://127.0.0.1:8787/api/schedule/cancel
```

### Check for a manager update and apply it

```bash
curl -X POST http://127.0.0.1:8787/api/update/check
curl -X POST http://127.0.0.1:8787/api/update/apply
```

> **Note on stdin commands:** many Windows game servers read from the Windows console input buffer rather than their stdin pipe. Commands are forwarded to the pipe on a best-effort basis; whether the server acts on them depends on its own I/O implementation.
