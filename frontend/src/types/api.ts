// ──────────────────────────────────────────────────────────────────────────────
// API models matching the Rust backend structs exactly.
// ──────────────────────────────────────────────────────────────────────────────

export type ServerStatus = 'stopped' | 'starting' | 'running' | 'stopping' | 'crashed'

export interface ServerInfo {
  status: ServerStatus
  pid: number | null
  uptime_seconds: number | null
  started_at: string | null
}

export interface ServerConfig {
  server_name: string
  max_players: number
  port: number
  invite_code: string | null
  [key: string]: unknown
}

export interface WorldConfig {
  world_name: string
  seed: string | null
  [key: string]: unknown
}

export interface Player {
  name: string
  joined_at: string
}

export type PlayerEventKind = 'joined' | 'left'

export interface PlayerEvent {
  player_name: string
  kind: PlayerEventKind
  timestamp: string
}

export type LogLevel = 'INFO' | 'WARN' | 'ERROR' | 'DEBUG' | 'UNKNOWN'

export interface LogLine {
  timestamp: string
  level: LogLevel
  message: string
  raw: string
}

export type BackupJobState = 'idle' | 'running' | 'done' | 'failed'

export interface BackupEntry {
  id: string
  created_at: string
  path: string
  size_bytes: number
  label: string | null
}

export interface BackupStatus {
  job_state: BackupJobState
  progress_pct: number | null
  current_file: string | null
  history: BackupEntry[]
  last_error: string | null
}

export interface ScheduleConfig {
  enabled: boolean
  restart_hour: number
  restart_minute: number
  warning_seconds: number
}

export interface ScheduleState {
  config: ScheduleConfig
  countdown_active: boolean
  countdown_seconds_remaining: number | null
  last_restart_date: string | null
}

export type InstallJobState = 'idle' | 'detecting' | 'detected' | 'installing' | 'done' | 'failed'

export interface InstallState {
  job_state: InstallJobState
  progress_pct: number | null
  current_file: string | null
  detected_sources: string[]
  destination: string | null
  last_error: string | null
}

export type UpdateCheckState = 'idle' | 'checking' | 'done' | 'failed'

export interface UpdateState {
  current_version: string
  latest_version: string | null
  update_available: boolean
  last_checked_at: string | null
  check_state: UpdateCheckState
  release_notes: string | null
  download_url: string | null
}

export interface AppStateSnapshot {
  server: ServerInfo
  server_config: ServerConfig | null
  world_config: WorldConfig | null
  recent_logs: LogLine[]
  players: Player[]
  player_count: number
  player_events: PlayerEvent[]
  app_version: string
  snapshot_at: string
  backup: BackupStatus
  schedule: ScheduleState
  install: InstallState
  update: UpdateState
}

export interface ApiResponse<T> {
  success: boolean
  data: T | null
  message: string | null
}

// ──────────────────────────────────────────────────────────────────────────────
// WebSocket event types
// ──────────────────────────────────────────────────────────────────────────────

export type WsEvent =
  | { event: 'server_status_changed'; data: ServerInfo }
  | { event: 'log_line'; data: LogLine }
  | { event: 'player_joined'; data: { player_name: string } }
  | { event: 'player_left'; data: { player_name: string } }
  | { event: 'notification'; data: { level: string; message: string } }
  | { event: 'ping'; data: null }
  | { event: 'backup_progress'; data: { job_state: string; progress_pct: number | null; current_file: string | null; entry: BackupEntry | null } }
  | { event: 'schedule_countdown'; data: { seconds_remaining: number; cancelled: boolean } }
  | { event: 'install_progress'; data: { job_state: string; progress_pct: number | null; current_file: string | null } }
  | { event: 'update_available'; data: { current_version: string; latest_version: string; download_url: string | null } }
