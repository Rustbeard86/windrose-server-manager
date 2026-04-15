import { useEffect, useState } from 'react'
import type { SetupStatus, SetupApply, ManagerConfig } from '../types/api'
import { apiGet, apiPut } from '../utils/api'
import './SetupWizard.css'

interface SetupWizardProps {
  onComplete: () => void
}

export function SetupWizard({ onComplete }: SetupWizardProps) {
  const [loading, setLoading] = useState(true)
  const [saving, setSaving] = useState(false)
  const [status, setStatus] = useState<{ kind: 'ok' | 'err'; msg: string } | null>(null)

  const [serverExecutable, setServerExecutable] = useState('')
  const [serverWorkingDir, setServerWorkingDir] = useState('')
  const [logFilePath, setLogFilePath] = useState('')
  const [bindAddress, setBindAddress] = useState('127.0.0.1')
  const [port, setPort] = useState(8787)
  const [enableLog, setEnableLog] = useState(true)
  const [autoDetected, setAutoDetected] = useState(false)

  useEffect(() => {
    async function load() {
      try {
        const res = await apiGet<SetupStatus>('/api/setup/status')
        if (res.success && res.data) {
          const cfg: ManagerConfig = res.data.config

          // If the backend detected a server nearby, pre-fill with those paths.
          if (res.data.detected_executable) {
            setServerExecutable(res.data.detected_executable)
            setServerWorkingDir(res.data.detected_working_dir ?? '')
            setLogFilePath(res.data.detected_log_file ?? '')
            setAutoDetected(true)
          } else {
            setServerExecutable(cfg.server_executable ?? '')
            setServerWorkingDir(cfg.server_working_dir ?? '')
            setLogFilePath(cfg.log_file_path ?? '')
          }

          setBindAddress(cfg.bind_address)
          setPort(cfg.port)
          setEnableLog(cfg.server_args.includes('-log'))
        }
      } catch {
        // defaults are already set
      } finally {
        setLoading(false)
      }
    }
    load()
  }, [])

  async function handleSave() {
    setSaving(true)
    setStatus(null)
    try {
      const body: SetupApply = {
        server_executable: serverExecutable || undefined,
        server_working_dir: serverWorkingDir || undefined,
        log_file_path: logFilePath || undefined,
        bind_address: bindAddress,
        port,
        server_args: enableLog ? ['-log'] : [],
      }
      const res = await apiPut<ManagerConfig>('/api/setup/config', body)
      if (res.success) {
        setStatus({ kind: 'ok', msg: 'Configuration saved. Loading dashboard…' })
        setTimeout(onComplete, 600)
      } else {
        setStatus({ kind: 'err', msg: res.message || 'Failed to save configuration.' })
      }
    } catch (e) {
      setStatus({ kind: 'err', msg: `Error: ${e instanceof Error ? e.message : e}` })
    } finally {
      setSaving(false)
    }
  }

  if (loading) {
    return (
      <div className="setup-overlay">
        <p className="text-muted">Loading configuration…</p>
      </div>
    )
  }

  return (
    <div className="setup-overlay">
      <div className="setup-card">
        <div className="setup-header">
          <svg
            className="compass-rose spin-slow"
            viewBox="0 0 40 40"
            fill="none"
            xmlns="http://www.w3.org/2000/svg"
            aria-hidden="true"
          >
            <circle cx="20" cy="20" r="18" stroke="#c9a84c" strokeWidth="1.5" strokeDasharray="4 2" />
            <polygon points="20,4 22,20 20,22 18,20" fill="#c9a84c" />
            <polygon points="20,36 22,20 20,18 18,20" fill="#7a8fa6" />
            <polygon points="4,20 20,18 22,20 20,22" fill="#4ab8c8" />
            <polygon points="36,20 20,22 18,20 20,18" fill="#7a8fa6" />
            <circle cx="20" cy="20" r="2.5" fill="#c9a84c" />
          </svg>
          <h1 className="setup-title">Windrose Server Manager</h1>
          <p className="setup-subtitle">
            Welcome! Configure the paths to your Windrose dedicated server below.
            Place this manager binary in the same folder as <strong>WindroseServer.exe</strong> for
            the simplest setup — relative paths will resolve automatically.
          </p>
        </div>

        {autoDetected && (
          <div className="setup-detected">
            Server installation detected nearby — paths have been pre-filled.
          </div>
        )}

        <div className="setup-fields">
          <div className="setup-field">
            <label className="setup-field-label">Server Executable</label>
            <input
              className="input"
              type="text"
              value={serverExecutable}
              onChange={(e) => setServerExecutable(e.target.value)}
              placeholder="WindroseServer.exe"
            />
            <span className="setup-field-hint">
              Path to WindroseServer.exe — relative to this binary, or an absolute path.
            </span>
          </div>

          <div className="setup-field">
            <label className="setup-field-label">Server Working Directory</label>
            <input
              className="input"
              type="text"
              value={serverWorkingDir}
              onChange={(e) => setServerWorkingDir(e.target.value)}
              placeholder="R5"
            />
            <span className="setup-field-hint">
              Directory containing ServerDescription.json and the Saved folder.
            </span>
          </div>

          <div className="setup-field">
            <label className="setup-field-label">Log File Path</label>
            <input
              className="input"
              type="text"
              value={logFilePath}
              onChange={(e) => setLogFilePath(e.target.value)}
              placeholder="R5\Saved\Logs\R5.log"
            />
            <span className="setup-field-hint">
              Path to the server log file the manager will tail.
            </span>
          </div>

          <div className="setup-field">
            <label className="setup-toggle-row">
              <input
                type="checkbox"
                checked={enableLog}
                onChange={(e) => setEnableLog(e.target.checked)}
              />
              <span>Launch server with <code>-log</code> flag</span>
            </label>
            <span className="setup-field-hint">
              Enables verbose server logging. Recommended for troubleshooting.
            </span>
          </div>

          <div className="setup-row">
            <div className="setup-field">
              <label className="setup-field-label">Bind Address</label>
              <input
                className="input"
                type="text"
                value={bindAddress}
                onChange={(e) => setBindAddress(e.target.value)}
                placeholder="127.0.0.1"
              />
              <span className="setup-field-hint">
                Use 0.0.0.0 to allow remote access.
              </span>
            </div>
            <div className="setup-field">
              <label className="setup-field-label">Port</label>
              <input
                className="input"
                type="number"
                min={1}
                max={65535}
                value={port}
                onChange={(e) => setPort(parseInt(e.target.value, 10) || 8787)}
              />
            </div>
          </div>
        </div>

        {status && (
          <div className={`setup-status ${status.kind === 'ok' ? 'setup-status--ok' : 'setup-status--err'}`}>
            {status.msg}
          </div>
        )}

        <div className="setup-actions">
          <button
            className="btn btn-primary"
            onClick={handleSave}
            disabled={saving}
          >
            {saving ? 'Saving…' : 'Save & Continue'}
          </button>
        </div>
      </div>
    </div>
  )
}
