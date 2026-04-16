import { useState } from 'react'
import type { AppStateSnapshot, ServerStats } from '../types/api'
import { apiPost } from '../utils/api'
import { formatUptime, formatDateTime } from '../utils/format'
import { StatusBadge } from '../components/StatusBadge'
import './DashboardView.css'

interface DashboardViewProps {
  state: AppStateSnapshot
  onReload: () => void
  canManageServer: boolean
}

export function DashboardView({ state, onReload, canManageServer }: DashboardViewProps) {
  const [cmdInput, setCmdInput] = useState('')
  const [cmdResult, setCmdResult] = useState<string | null>(null)
  const [sending, setSending] = useState(false)

  const { server, server_config, players, player_count, player_events, update } = state

  const isRunning = server.status === 'running'
  const isBusy = server.status === 'starting' || server.status === 'stopping'

  async function handleLifecycle(action: 'start' | 'stop' | 'restart') {
    try {
      await apiPost(`/api/server/${action}`)
      setTimeout(onReload, 400)
    } catch (e) {
      console.error(e)
    }
  }

  async function sendCommand(e: React.FormEvent) {
    e.preventDefault()
    if (!cmdInput.trim()) return
    setSending(true)
    setCmdResult(null)
    try {
      const res = await apiPost<{ sent: boolean }>('/api/server/command', {
        command: cmdInput.trim(),
      })
      setCmdResult(res.success ? '✓ Command sent' : `✗ ${res.message}`)
      setCmdInput('')
    } catch (err) {
      setCmdResult(`✗ ${err instanceof Error ? err.message : 'Error'}`)
    } finally {
      setSending(false)
    }
  }

  async function sendPresetCommand(command: string) {
    setSending(true)
    setCmdResult(null)
    try {
      const res = await apiPost<{ sent: boolean }>('/api/server/command', { command })
      setCmdResult(res.success ? `✓ ${command}` : `✗ ${res.message}`)
    } catch (err) {
      setCmdResult(`✗ ${err instanceof Error ? err.message : 'Error'}`)
    } finally {
      setSending(false)
    }
  }

  return (
    <div className="dashboard-view animate-fade-in">
      {/* ── Hero / status region ─────────────────────────────────────── */}
      <section className="hero-region card">
        <div className="hero-left">
          <div className="hero-server-name">
            {server_config?.server_name || 'Windrose Server'}
          </div>
          <div className="hero-meta">
            <StatusBadge status={server.status} />
            {server.pid && (
              <span className="text-faint" style={{ fontSize: '0.72rem' }}>PID {server.pid}</span>
            )}
          </div>
        </div>

        <div className="hero-stats">
          <div className="hero-stat">
            <span className="label-xs">Uptime</span>
            <span className="hero-stat-value text-teal">
              {server.uptime_seconds != null ? formatUptime(server.uptime_seconds) : '—'}
            </span>
          </div>
          <div className="hero-stat">
            <span className="label-xs">Players</span>
            <span className="hero-stat-value text-gold">{player_count}</span>
          </div>
          <div className="hero-stat">
            <span className="label-xs">Port</span>
            <span className="hero-stat-value">{server_config?.port || '—'}</span>
          </div>
          <div className="hero-stat">
            <span className="label-xs">Max Players</span>
            <span className="hero-stat-value">{server_config?.max_players || '—'}</span>
          </div>
        </div>

        {/* Controls */}
        <div className="hero-controls">
          <div className="hero-invite-code">
            <span className="text-muted" style={{ fontSize: '0.7rem', marginRight: '4px' }}>JOIN CODE</span>
            <code>{server_config?.invite_code || 'pending...'}</code>
          </div>
          <button
            className="btn btn-primary"
            onClick={() => handleLifecycle('start')}
            disabled={!canManageServer || isRunning || isBusy}
          >
            ▶ Start
          </button>
          <button
            className="btn btn-danger"
            onClick={() => handleLifecycle('stop')}
            disabled={!canManageServer || !isRunning || isBusy}
          >
            ■ Stop
          </button>
          <button
            className="btn"
            onClick={() => handleLifecycle('restart')}
            disabled={!canManageServer || isBusy}
          >
            ↺ Restart
          </button>
        </div>
      </section>

      {/* Update available banner */}
      {update.update_available && update.latest_version && (
        <div className="update-banner">
          <span>⬆ Update available: v{update.latest_version}</span>
          {update.download_url && (
            <a
              href={update.download_url}
              target="_blank"
              rel="noreferrer"
              className="btn btn-sm btn-primary"
            >
              Download
            </a>
          )}
        </div>
      )}

      <div className="dashboard-grid">
        {/* ── Command console ────────────────────────────────────────── */}
        <div className="card dashboard-card">
          <div className="panel-title">
            <span className="panel-title-icon">⌨</span>
            Console Command
          </div>
          <form className="cmd-form" onSubmit={sendCommand}>
            <input
              className="input input-mono"
              type="text"
              placeholder="Type a server command…"
              value={cmdInput}
              onChange={(e) => setCmdInput(e.target.value)}
              disabled={!canManageServer || !isRunning || sending}
            />
            <button
              className="btn btn-primary"
              type="submit"
              disabled={!canManageServer || !isRunning || sending || !cmdInput.trim()}
            >
              Send
            </button>
          </form>
          <div className="hero-controls" style={{ marginTop: '0.65rem', justifyContent: 'flex-start' }}>
            <button className="btn btn-sm" onClick={() => void sendPresetCommand('save world')} disabled={!canManageServer || !isRunning || sending}>
              Save World
            </button>
            <button className="btn btn-sm" onClick={() => void sendPresetCommand('list players')} disabled={!canManageServer || !isRunning || sending}>
              List Players
            </button>
            <button className="btn btn-sm" onClick={() => void sendPresetCommand('logs')} disabled={!canManageServer || !isRunning || sending}>
              Show Logs
            </button>
            <button className="btn btn-sm btn-danger" onClick={() => void sendPresetCommand('quit')} disabled={!canManageServer || !isRunning || sending}>
              Quit Server
            </button>
          </div>
          {cmdResult && (
            <div className={`cmd-result ${cmdResult.startsWith('✓') ? 'text-success' : 'text-danger'}`}>
              {cmdResult}
            </div>
          )}
          {!isRunning && (
            <p className="text-faint" style={{ fontSize: '0.75rem', marginTop: '0.5rem' }}>
              Server must be running to send commands
            </p>
          )}
        </div>

        {/* ── Recent activity ─────────────────────────────────────────── */}
        <div className="card dashboard-card">
          <div className="panel-title">
            <span className="panel-title-icon">🔔</span>
            Recent Activity
          </div>
          <div className="activity-feed">
            {player_events.length === 0 ? (
              <p className="text-faint" style={{ fontSize: '0.8rem' }}>No recent activity</p>
            ) : (
              [...player_events].reverse().slice(0, 12).map((ev, i) => (
                <div key={i} className="activity-item animate-fade-in">
                  <span className={`activity-dot ${ev.kind === 'joined' ? 'dot-running' : 'dot-stopped'} dot`} />
                  <span className="activity-name">{ev.player_name}</span>
                  <span className={`activity-kind ${ev.kind === 'joined' ? 'text-success' : 'text-muted'}`}>
                    {ev.kind}
                  </span>
                  <span className="activity-time text-faint">{formatDateTime(ev.timestamp)}</span>
                </div>
              ))
            )}
          </div>
        </div>

        {/* ── Online players ──────────────────────────────────────────── */}
        <div className="card dashboard-card">
          <div className="panel-title">
            <span className="panel-title-icon">👥</span>
            Online — {player_count}
          </div>
          {players.length === 0 ? (
            <p className="text-faint" style={{ fontSize: '0.8rem' }}>No players online</p>
          ) : (
            <ul className="player-list">
              {players.map((p) => (
                <li key={p.name} className="player-item">
                  <span className="dot dot-running" style={{ flexShrink: 0 }} />
                  <span className="player-name">{p.name}</span>
                  <span className="player-since text-faint">{formatDateTime(p.joined_at)}</span>
                </li>
              ))}
            </ul>
          )}
        </div>

        {/* ── Quick info ───────────────────────────────────────────────── */}
        <div className="card dashboard-card">
          <div className="panel-title">
            <span className="panel-title-icon">ℹ</span>
            Server Info
          </div>
          <dl className="info-list">
            <dt>World</dt>
            <dd>{state.world_config?.world_name || state.world_config?.seed || '—'}</dd>
            <dt>Seed</dt>
            <dd>{state.world_config?.seed || '—'}</dd>
            <dt>Started</dt>
            <dd>{server.started_at ? formatDateTime(server.started_at) : '—'}</dd>
            <dt>Manager</dt>
            <dd className="text-teal">v{state.app_version}</dd>
            <dt>Backup</dt>
            <dd className={state.backup.job_state === 'running' ? 'text-warning' : ''}>
              {state.backup.job_state === 'running'
                ? `Running ${state.backup.progress_pct ?? 0}%`
                : state.backup.history.length > 0
                  ? `Last: ${formatDateTime(state.backup.history[state.backup.history.length - 1].created_at)}`
                  : 'None'}
            </dd>
          </dl>
        </div>
      </div>

      {/* ── Stats panel ─────────────────────────────────────────────────── */}
      {state.server_stats && <StatsPanel stats={state.server_stats} />}
    </div>
  )
}

// ── Helpers ───────────────────────────────────────────────────────────────────

function formatBytes(bytes: number): string {
  if (bytes >= 1_073_741_824) return `${(bytes / 1_073_741_824).toFixed(1)} GB`
  if (bytes >= 1_048_576) return `${(bytes / 1_048_576).toFixed(0)} MB`
  if (bytes >= 1_024) return `${(bytes / 1_024).toFixed(0)} KB`
  return `${bytes} B`
}

// ── StatsPanel ────────────────────────────────────────────────────────────────

const GAUGE_R = 38
const GAUGE_CIRC = 2 * Math.PI * GAUGE_R

function gaugeColor(pct: number): string {
  if (pct >= 80) return 'var(--danger)'
  if (pct >= 50) return 'var(--warning)'
  return 'var(--teal)'
}

interface GaugeProps {
  label: string
  value: number   // 0-100
  display: string // text shown in centre
  unit?: string
}

function CircleGauge({ label, value, display, unit }: GaugeProps) {
  const pct = Math.max(0, Math.min(100, value))
  const offset = GAUGE_CIRC * (1 - pct / 100)
  const color = gaugeColor(pct)
  return (
    <div className="stats-gauge">
      <svg viewBox="0 0 100 100" className="gauge-svg">
        {/* background track */}
        <circle cx="50" cy="50" r={GAUGE_R} fill="none" stroke="var(--bg)" strokeWidth="11" />
        {/* value arc */}
        <circle
          cx="50" cy="50" r={GAUGE_R}
          fill="none"
          stroke={color}
          strokeWidth="11"
          strokeDasharray={`${GAUGE_CIRC} ${GAUGE_CIRC}`}
          strokeDashoffset={offset}
          strokeLinecap="round"
          transform="rotate(-90 50 50)"
        />
      </svg>
      <div className="gauge-center">
        <span className="gauge-value" style={{ color }}>{display}</span>
        {unit && <span className="gauge-unit">{unit}</span>}
      </div>
      <div className="gauge-label">{label}</div>
    </div>
  )
}

function StatsPanel({ stats }: { stats: ServerStats }) {
  const cpuPct = stats.cpu_percent
  const ramPct = stats.memory_total_bytes > 0
    ? (stats.memory_bytes / stats.memory_total_bytes) * 100
    : 0

  return (
    <section className="card stats-panel">
      <div className="panel-title" style={{ marginBottom: '1rem' }}>
        <span className="panel-title-icon">📊</span>
        Resource Usage
        <span className="text-faint" style={{ marginLeft: 'auto', fontSize: '0.72rem' }}>
          live · process
        </span>
      </div>
      <div className="stats-grid">
        {/* CPU gauge */}
        <CircleGauge
          label="CPU"
          value={cpuPct}
          display={cpuPct.toFixed(1)}
          unit="%"
        />
        {/* RAM gauge */}
        <CircleGauge
          label="RAM"
          value={ramPct}
          display={formatBytes(stats.memory_bytes).replace(' ', '\u00a0')}
        />
        {/* Disk */}
        <div className="stats-text-card">
          <div className="stats-text-value">{formatBytes(stats.disk_used_bytes)}</div>
          <div className="stats-text-label">Disk (server folder)</div>
        </div>
        {/* Network */}
        <div className="stats-text-card">
          <div className="stats-text-value">
            ↓ {formatBytes(stats.net_rx_bytes_per_sec)}/s
          </div>
          <div className="stats-text-value">
            ↑ {formatBytes(stats.net_tx_bytes_per_sec)}/s
          </div>
          <div className="stats-text-label">Network (system)</div>
        </div>
      </div>
    </section>
  )
}
