import { useState } from 'react'
import type { AppStateSnapshot } from '../types/api'
import { apiPost } from '../utils/api'
import { formatUptime, formatDateTime } from '../utils/format'
import { StatusBadge } from '../components/StatusBadge'
import './DashboardView.css'

interface DashboardViewProps {
  state: AppStateSnapshot
  onReload: () => void
}

export function DashboardView({ state, onReload }: DashboardViewProps) {
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
            {server_config?.invite_code && (
              <span className="hero-invite-code">
                <span className="text-muted" style={{ fontSize: '0.7rem', marginRight: '4px' }}>INVITE</span>
                <code>{server_config.invite_code}</code>
              </span>
            )}
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
          <button
            className="btn btn-primary"
            onClick={() => handleLifecycle('start')}
            disabled={isRunning || isBusy}
          >
            ▶ Start
          </button>
          <button
            className="btn btn-danger"
            onClick={() => handleLifecycle('stop')}
            disabled={!isRunning || isBusy}
          >
            ■ Stop
          </button>
          <button
            className="btn"
            onClick={() => handleLifecycle('restart')}
            disabled={isBusy}
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
              disabled={!isRunning || sending}
            />
            <button
              className="btn btn-primary"
              type="submit"
              disabled={!isRunning || sending || !cmdInput.trim()}
            >
              Send
            </button>
          </form>
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
            <dd>{state.world_config?.world_name || '—'}</dd>
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
    </div>
  )
}
