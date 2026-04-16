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
export type UpdateApplyState = 'idle' | 'downloading' | 'applying' | 'pending_restart' | 'failed'

export interface UpdateState {
  current_version: string
  latest_version: string | null
  update_available: boolean
  last_checked_at: string | null
  check_state: UpdateCheckState
  apply_state: UpdateApplyState
  release_notes: string | null
  download_url: string | null
}

export interface ServerStats {
  /** Process CPU usage 0–100 %, normalised across all logical CPUs. */
  cpu_percent: number
  /** Process resident-set size in bytes. */
  memory_bytes: number
  /** System total physical memory in bytes. */
  memory_total_bytes: number
  /** Cumulative size of all files under the server folder, in bytes. */
  disk_used_bytes: number
  /** System-wide network receive bytes since last sample. */
  net_rx_bytes_per_sec: number
  /** System-wide network transmit bytes since last sample. */
  net_tx_bytes_per_sec: number
  collected_at: string
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
  /** `true` when a server executable path is configured and the file exists. */
  server_configured: boolean
  /** Live resource stats; `null` when the server is not running. */
  server_stats: ServerStats | null
}

export interface ApiResponse<T> {
  success: boolean
  data: T | null
  message: string | null
}

// ──────────────────────────────────────────────────────────────────────────────
// Auth types
// ──────────────────────────────────────────────────────────────────────────────

export interface AuthStatus {
  has_users: boolean
  needs_bootstrap: boolean
}

export interface SessionInfo {
  username: string
  is_admin: boolean
  permission_flags: number
}

export interface AuthUserSummary {
  id: number
  username: string
  is_admin: boolean
  permission_flags: number
  disabled: boolean
  created_at: number
}

export interface InviteSummary {
  id: number
  permission_flags: number
  max_uses: number
  uses: number
  created_at: number
  expires_at: number | null
  created_by_user: number | null
  exhausted: boolean
  expired: boolean
}

export interface CreatedInvite {
  code: string
  permission_flags: number
  max_uses: number
  expires_at: number | null
}

export interface AuditEventSummary {
  id: number
  created_at: number
  actor_user_id: number | null
  actor_username: string | null
  action: string
  details: string | null
  success: boolean
}

// ──────────────────────────────────────────────────────────────────────────────
// Setup / FTUE types
// ──────────────────────────────────────────────────────────────────────────────

export interface ManagerConfig {
  bind_address: string
  port: number
  server_executable: string | null
  server_args: string[]
  server_working_dir: string | null
  log_file_path: string | null
  log_buffer_capacity: number
  server_stop_timeout_secs: number
  player_event_capacity: number
  backup_dir: string
  history_file_path: string | null
  update_check_url: string
}

export interface SetupStatus {
  needs_setup: boolean
  config: ManagerConfig
  detected_executable: string | null
  detected_working_dir: string | null
  detected_log_file: string | null
}

export interface SetupApply {
  bind_address?: string
  port?: number
  server_executable?: string
  server_working_dir?: string
  log_file_path?: string
  server_args?: string[]
}

// ──────────────────────────────────────────────────────────────────────────────
// Config file management types
// ──────────────────────────────────────────────────────────────────────────────

export type ConfigFileKind = 'server_description' | 'world_description'

export interface ConfigFileInfo {
  path: string
  file_name: string
  kind: ConfigFileKind
  last_modified: string
}

export interface ConfigFileContent {
  path: string
  content: string
  last_modified: string
}

export interface ConfigFileWrite {
  path: string
  content: string
  last_modified: string
}

export interface ValidateResponse {
  valid: boolean
  error: string | null
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
  | { event: 'stats_updated'; data: ServerStats }
