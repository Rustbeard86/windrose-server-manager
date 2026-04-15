import { useState } from 'react'
import type { AppStateSnapshot } from '../types/api'
import { apiPost, apiPut } from '../utils/api'
import { formatBytes, formatDateTime } from '../utils/format'
import './OperationsView.css'

interface OperationsViewProps {
  state: AppStateSnapshot
  onReload: () => void
}

export function OperationsView({ state, onReload }: OperationsViewProps) {
  const { backup, schedule, install, update } = state

  return (
    <div className="ops-view animate-fade-in">
      <BackupPanel backup={backup} onReload={onReload} />
      <SchedulePanel schedule={schedule} onReload={onReload} />
      <UpdatePanel update={update} onReload={onReload} />
      <InstallPanel install={install} onReload={onReload} />
    </div>
  )
}

// ──────────────────────────────────────────────────────────────────────────────
// Backup panel
// ──────────────────────────────────────────────────────────────────────────────
function BackupPanel({ backup, onReload }: { backup: AppStateSnapshot['backup']; onReload: () => void }) {
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
          disabled={running || creating}
        />
        <button
          className="btn btn-primary"
          onClick={createBackup}
          disabled={running || creating}
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
function SchedulePanel({ schedule, onReload }: { schedule: AppStateSnapshot['schedule']; onReload: () => void }) {
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
              onChange={(e) => setCfg((c) => ({ ...c, restart_hour: parseInt(e.target.value, 10) }))}
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
              onChange={(e) => setCfg((c) => ({ ...c, restart_minute: parseInt(e.target.value, 10) }))}
            />
          </div>
          <div className="field-group">
            <label className="field-label">Warning (seconds)</label>
            <input
              className="input"
              type="number"
              min={0}
              value={cfg.warning_seconds}
              onChange={(e) => setCfg((c) => ({ ...c, warning_seconds: parseInt(e.target.value, 10) }))}
            />
          </div>
        </div>
      </div>

      <div className="ops-form" style={{ marginTop: '0.75rem' }}>
        <button className="btn btn-primary" onClick={save} disabled={saving}>
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
function UpdatePanel({ update, onReload }: { update: AppStateSnapshot['update']; onReload: () => void }) {
  const [checking, setChecking] = useState(false)

  async function checkUpdate() {
    setChecking(true)
    try {
      await apiPost('/api/update/check')
      setTimeout(onReload, 800)
    } catch (e) {
      console.error(e)
    } finally {
      setTimeout(() => setChecking(false), 1000)
    }
  }

  return (
    <div className="card ops-panel">
      <div className="panel-title">
        <span className="panel-title-icon">⬆</span>
        App Update
        {update.update_available && (
          <span className="badge badge-starting" style={{ marginLeft: 'auto' }}>Update Available</span>
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

      <div className="ops-form" style={{ marginTop: '0.75rem' }}>
        <button className="btn" onClick={checkUpdate} disabled={checking || update.check_state === 'checking'}>
          {checking || update.check_state === 'checking' ? 'Checking…' : '🔍 Check for Updates'}
        </button>
        {update.update_available && update.download_url && (
          <a href={update.download_url} target="_blank" rel="noreferrer" className="btn btn-primary">
            Download Latest
          </a>
        )}
      </div>
    </div>
  )
}

// ──────────────────────────────────────────────────────────────────────────────
// Install panel
// ──────────────────────────────────────────────────────────────────────────────
function InstallPanel({ install, onReload }: { install: AppStateSnapshot['install']; onReload: () => void }) {
  const [source, setSource] = useState('')
  const [dest, setDest] = useState('')
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
      </div>

      <div className="ops-form">
        <button className="btn" onClick={detect} disabled={detecting || isBusy}>
          {detecting ? 'Detecting…' : '🔍 Detect Steam'}
        </button>
        <button
          className="btn btn-primary"
          onClick={runInstall}
          disabled={!source || !dest || installing || isBusy}
        >
          {installing ? 'Installing…' : '▶ Run Install'}
        </button>
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
