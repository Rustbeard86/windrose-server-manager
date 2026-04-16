import { useState } from 'react'
import type { AppStateSnapshot } from '../types/api'
import { apiPost, apiPut } from '../utils/api'
import { formatBytes, formatDateTime } from '../utils/format'
import './OperationsView.css'

interface OperationsViewProps {
  state: AppStateSnapshot
  onReload: () => void
  canManageBackups: boolean
  canManageSchedule: boolean
  canManageUpdates: boolean
  canManageInstall: boolean
}

export function OperationsView({
  state,
  onReload,
  canManageBackups,
  canManageSchedule,
  canManageUpdates,
  canManageInstall,
}: OperationsViewProps) {
  const { backup, schedule, install, update, server_configured } = state

  return (
    <div className="ops-view animate-fade-in">
      <BackupPanel backup={backup} onReload={onReload} canManage={canManageBackups} />
      <SchedulePanel schedule={schedule} onReload={onReload} canManage={canManageSchedule} />
      <UpdatePanel update={update} onReload={onReload} canManage={canManageUpdates} />
      <InstallPanel install={install} serverConfigured={server_configured} onReload={onReload} canManage={canManageInstall} />
    </div>
  )
}

// ──────────────────────────────────────────────────────────────────────────────
// Backup panel
// ──────────────────────────────────────────────────────────────────────────────
function BackupPanel({ backup, onReload, canManage }: { backup: AppStateSnapshot['backup']; onReload: () => void; canManage: boolean }) {
  const [label, setLabel] = useState('')
  const [creating, setCreating] = useState(false)
  const [msg, setMsg] = useState<string | null>(null)
  const running = backup.job_state === 'running'

  async function createBackup() {
    setCreating(true)
    setMsg(null)
    try {
      const res = await apiPost('/api/backup/create', label ? { label } : undefined)
      if (res.success) {
        setMsg('Backup started')
        setLabel('')
        setTimeout(onReload, 500)
      } else {
        setMsg(`Failed: ${res.message}`)
      }
    } catch (e) {
      setMsg(`Error: ${e instanceof Error ? e.message : e}`)
    } finally {
      setCreating(false)
    }
  }

  return (
    <div className="card ops-panel">
      <div className="panel-title">
        <span className="panel-title-icon">💾</span>
        Backups
        <span className={`badge ${running ? 'badge-starting' : 'badge-stopped'}`} style={{ marginLeft: 'auto' }}>
          {running ? `Running ${backup.progress_pct ?? 0}%` : backup.job_state}
        </span>
      </div>

      {running && (
        <div className="ops-progress">
          <div className="progress-bar">
            <div className="progress-bar-fill" style={{ width: `${backup.progress_pct ?? 0}%` }} />
          </div>
          {backup.current_file && (
            <p className="ops-progress-file text-faint font-mono">{backup.current_file}</p>
          )}
        </div>
      )}

      <div className="ops-form">
        <input
          className="input"
          type="text"
          placeholder="Optional label…"
          value={label}
          onChange={(e) => setLabel(e.target.value)}
          disabled={!canManage || running || creating}
        />
        <button
          className="btn btn-primary"
          onClick={createBackup}
          disabled={!canManage || running || creating}
        >
          {creating ? 'Starting…' : '+ Create Backup'}
        </button>
      </div>

      {msg && (
        <p className={`ops-msg ${msg.startsWith('Backup') ? 'text-success' : 'text-danger'}`}>{msg}</p>
      )}

      {backup.last_error && (
        <p className="ops-msg text-danger">Last error: {backup.last_error}</p>
      )}

      {backup.history.length > 0 && (
        <div className="ops-history">
          <div className="label-xs" style={{ marginBottom: '0.5rem' }}>History</div>
          {[...backup.history].reverse().map((entry) => (
            <div key={entry.id} className="backup-entry">
              <span className="backup-label">{entry.label || 'Backup'}</span>
              <span className="text-muted" style={{ fontSize: '0.78rem' }}>{formatDateTime(entry.created_at)}</span>
              <span className="text-teal font-mono" style={{ fontSize: '0.78rem' }}>{formatBytes(entry.size_bytes)}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}

// ──────────────────────────────────────────────────────────────────────────────
// Schedule panel
// ──────────────────────────────────────────────────────────────────────────────
function SchedulePanel({ schedule, onReload, canManage }: { schedule: AppStateSnapshot['schedule']; onReload: () => void; canManage: boolean }) {
  const [cfg, setCfg] = useState(schedule.config)
  const [saving, setSaving] = useState(false)
  const [msg, setMsg] = useState<string | null>(null)

  async function save() {
    setSaving(true)
    setMsg(null)
    try {
      const res = await apiPut('/api/schedule', cfg)
      if (res.success) {
        setMsg('Schedule saved.')
        onReload()
      } else {
        setMsg(`Failed: ${res.message}`)
      }
    } catch (e) {
      setMsg(`Error: ${e instanceof Error ? e.message : e}`)
    } finally {
      setSaving(false)
    }
  }

  async function cancelCountdown() {
    try {
      await apiPost('/api/schedule/cancel')
      onReload()
    } catch (e) {
      console.error(e)
    }
  }

  return (
    <div className="card ops-panel">
      <div className="panel-title">
        <span className="panel-title-icon">🕐</span>
        Scheduled Restart
        {schedule.countdown_active && (
          <span className="badge badge-starting" style={{ marginLeft: 'auto', animation: 'pulse-dot 1s ease-in-out infinite' }}>
            ⏳ {schedule.countdown_seconds_remaining ?? '?'}s remaining
          </span>
        )}
      </div>

      {schedule.countdown_active && (
        <div className="ops-countdown">
          <p className="text-warning">Restart countdown active!</p>
          <button className="btn btn-danger btn-sm" onClick={cancelCountdown}>Cancel Countdown</button>
        </div>
      )}

      <div className="ops-fields">
        <div className="toggle-row">
          <label className="field-label-inline">Enable scheduled restart</label>
          <label className="toggle-switch">
            <input
              type="checkbox"
              checked={cfg.enabled}
              disabled={!canManage}
              onChange={(e) => setCfg((c) => ({ ...c, enabled: e.target.checked }))}
            />
            <span className="toggle-slider" />
          </label>
        </div>

        <div className="field-row-3">
          <div className="field-group">
            <label className="field-label">Hour (0–23)</label>
            <input
              className="input"
              type="number"
              min={0}
              max={23}
              value={cfg.restart_hour}
              disabled={!canManage}
              onChange={(e) => setCfg((c) => ({ ...c, restart_hour: Math.min(23, Math.max(0, parseInt(e.target.value, 10) || 0)) }))}
            />
          </div>
          <div className="field-group">
            <label className="field-label">Minute (0–59)</label>
            <input
              className="input"
              type="number"
              min={0}
              max={59}
              value={cfg.restart_minute}
              disabled={!canManage}
              onChange={(e) => setCfg((c) => ({ ...c, restart_minute: Math.min(59, Math.max(0, parseInt(e.target.value, 10) || 0)) }))}
            />
          </div>
          <div className="field-group">
            <label className="field-label">Warning (seconds)</label>
            <input
              className="input"
              type="number"
              min={0}
              value={cfg.warning_seconds}
              disabled={!canManage}
              onChange={(e) => setCfg((c) => ({ ...c, warning_seconds: parseInt(e.target.value, 10) }))}
            />
          </div>
        </div>
      </div>

      <div className="ops-form" style={{ marginTop: '0.75rem' }}>
        <button className="btn btn-primary" onClick={save} disabled={!canManage || saving}>
          {saving ? 'Saving…' : 'Save Schedule'}
        </button>
      </div>

      {msg && (
        <p className={`ops-msg ${msg.includes('saved') ? 'text-success' : 'text-danger'}`}>{msg}</p>
      )}

      {schedule.last_restart_date && (
        <p className="text-faint" style={{ fontSize: '0.75rem', marginTop: '0.5rem' }}>
          Last restart: {schedule.last_restart_date}
        </p>
      )}
    </div>
  )
}

// ──────────────────────────────────────────────────────────────────────────────
// Update check panel
// ──────────────────────────────────────────────────────────────────────────────
function UpdatePanel({ update, onReload, canManage }: { update: AppStateSnapshot['update']; onReload: () => void; canManage: boolean }) {
  const [checking, setChecking] = useState(false)
  const [applying, setApplying] = useState(false)
  const [msg, setMsg] = useState<string | null>(null)

  const isApplying =
    applying ||
    update.apply_state === 'downloading' ||
    update.apply_state === 'applying' ||
    update.apply_state === 'pending_restart'

  async function checkUpdate() {
    setChecking(true)
    setMsg(null)
    try {
      const res = await apiPost('/api/update/check')
      if (!res.success) setMsg(res.message ?? 'Check failed')
      else setTimeout(onReload, 800)
    } catch (e) {
      setMsg(`Error: ${e instanceof Error ? e.message : e}`)
    } finally {
      setTimeout(() => setChecking(false), 1000)
    }
  }

  async function applyUpdate() {
    if (!confirm('Apply update? The manager will restart automatically. The game server will keep running.')) return
    setApplying(true)
    setMsg(null)
    try {
      const res = await apiPost('/api/update/apply')
      if (res.success) {
        setMsg('Update in progress — manager will restart shortly.')
      } else {
        setMsg(`Failed: ${res.message}`)
        setApplying(false)
      }
    } catch (e) {
      setMsg(`Error: ${e instanceof Error ? e.message : e}`)
      setApplying(false)
    }
  }

  const applyLabel =
    update.apply_state === 'downloading' ? 'Downloading…' :
    update.apply_state === 'applying' ? 'Applying…' :
    update.apply_state === 'pending_restart' ? 'Restarting…' :
    'Apply Update'

  return (
    <div className="card ops-panel">
      <div className="panel-title">
        <span className="panel-title-icon">⬆</span>
        App Update
        {update.update_available && (
          <span className="badge badge-starting" style={{ marginLeft: 'auto' }}>
            Update Available
          </span>
        )}
        {update.apply_state === 'failed' && (
          <span className="badge badge-stopped" style={{ marginLeft: 'auto' }}>
            Update Failed
          </span>
        )}
      </div>

      <div className="update-info">
        <div className="update-row">
          <span className="text-muted">Current version</span>
          <span className="font-mono text-teal">v{update.current_version}</span>
        </div>
        {update.latest_version && (
          <div className="update-row">
            <span className="text-muted">Latest version</span>
            <span className={`font-mono ${update.update_available ? 'text-warning' : 'text-success'}`}>
              v{update.latest_version}
            </span>
          </div>
        )}
        {update.last_checked_at && (
          <div className="update-row">
            <span className="text-muted">Last checked</span>
            <span className="text-faint" style={{ fontSize: '0.78rem' }}>
              {formatDateTime(update.last_checked_at)}
            </span>
          </div>
        )}
      </div>

      {msg && (
        <p className="text-muted" style={{ fontSize: '0.82rem', marginTop: '0.5rem' }}>{msg}</p>
      )}

      <div className="ops-form" style={{ marginTop: '0.75rem', gap: '0.5rem', display: 'flex', flexWrap: 'wrap' }}>
        <button
          className="btn"
          onClick={checkUpdate}
          disabled={!canManage || checking || update.check_state === 'checking' || isApplying}
        >
          {checking || update.check_state === 'checking' ? 'Checking…' : 'Check for Updates'}
        </button>

        {update.update_available && (
          <button
            className="btn btn-primary"
            onClick={applyUpdate}
            disabled={!canManage || isApplying}
          >
            {isApplying ? applyLabel : 'Apply Update'}
          </button>
        )}
      </div>

      {update.release_notes && (
        <details style={{ marginTop: '0.75rem' }}>
          <summary className="text-muted" style={{ cursor: 'pointer', fontSize: '0.82rem' }}>
            Release notes
          </summary>
          <pre style={{ fontSize: '0.75rem', whiteSpace: 'pre-wrap', marginTop: '0.4rem', color: 'var(--text-muted)' }}>
            {update.release_notes}
          </pre>
        </details>
      )}
    </div>
  )
}

// ──────────────────────────────────────────────────────────────────────────────
// Install panel
// ──────────────────────────────────────────────────────────────────────────────
function InstallPanel({ install, serverConfigured, onReload, canManage }: {
  install: AppStateSnapshot['install']
  serverConfigured: boolean
  onReload: () => void
  canManage: boolean
}) {
  const [source, setSource] = useState(install.detected_sources[0] ?? '')
  const [dest, setDest] = useState(install.destination ?? '')
  const [detecting, setDetecting] = useState(false)
  const [installing, setInstalling] = useState(false)
  const [msg, setMsg] = useState<string | null>(null)

  const isBusy = install.job_state === 'detecting' || install.job_state === 'installing'

  async function detect() {
    setDetecting(true)
    setMsg(null)
    try {
      await apiPost('/api/install/detect')
      setTimeout(onReload, 600)
    } catch (e) {
      setMsg(`Error: ${e instanceof Error ? e.message : e}`)
    } finally {
      setTimeout(() => setDetecting(false), 700)
    }
  }

  async function runInstall() {
    if (!source || !dest) return
    setInstalling(true)
    setMsg(null)
    try {
      const res = await apiPost('/api/install/run', { source, destination: dest })
      if (res.success) {
        setMsg('Install started.')
        onReload()
      } else {
        setMsg(`Failed: ${res.message}`)
      }
    } catch (e) {
      setMsg(`Error: ${e instanceof Error ? e.message : e}`)
    } finally {
      setInstalling(false)
    }
  }

  return (
    <div className="card ops-panel">
      <div className="panel-title">
        <span className="panel-title-icon">📦</span>
        Install / Detect
        {isBusy && (
          <span className="badge badge-starting" style={{ marginLeft: 'auto' }}>
            {install.job_state === 'detecting' ? 'Detecting…' : `Installing ${install.progress_pct ?? 0}%`}
          </span>
        )}
      </div>

      {isBusy && install.current_file && (
        <div className="ops-progress">
          <div className="progress-bar">
            <div className="progress-bar-fill" style={{ width: `${install.progress_pct ?? 0}%` }} />
          </div>
          <p className="ops-progress-file text-faint font-mono">{install.current_file}</p>
        </div>
      )}

      {install.detected_sources.length > 0 && (
        <div className="ops-detected">
          <div className="label-xs" style={{ marginBottom: '0.4rem' }}>Detected Sources</div>
          {install.detected_sources.map((s) => (
            <button
              key={s}
              className="btn btn-sm"
              style={{ marginBottom: '0.3rem', display: 'block', textAlign: 'left' }}
              onClick={() => setSource(s)}
            >
              {s}
            </button>
          ))}
        </div>
      )}

      <div className="ops-fields" style={{ marginTop: '0.75rem' }}>
        {!serverConfigured && (
          <>
            <div className="field-group">
              <label className="field-label">Source Path</label>
              <input
                className="input input-mono"
                type="text"
                value={source}
                onChange={(e) => setSource(e.target.value)}
                placeholder="C:\Steam\steamapps\common\WindroseServer"
              />
            </div>
            <div className="field-group">
              <label className="field-label">Destination Path</label>
              <input
                className="input input-mono"
                type="text"
                value={dest}
                onChange={(e) => setDest(e.target.value)}
                placeholder="C:\WindroseServer"
              />
            </div>
          </>
        )}
      </div>

      <div className="ops-form">
        <button className="btn" onClick={detect} disabled={!canManage || detecting || isBusy}>
          {detecting ? 'Detecting…' : '🔍 Detect'}
        </button>
        {!serverConfigured && (
          <button
            className="btn btn-primary"
            onClick={runInstall}
            disabled={!canManage || !source || !dest || installing || isBusy}
          >
            {installing ? 'Installing…' : '▶ Run Install'}
          </button>
        )}
      </div>

      {msg && (
        <p className={`ops-msg ${msg.includes('started') ? 'text-success' : 'text-danger'}`}>{msg}</p>
      )}

      {install.last_error && (
        <p className="ops-msg text-danger">Last error: {install.last_error}</p>
      )}
    </div>
  )
}
