import { useEffect, useState } from 'react'
import type { ServerConfig, WorldConfig } from '../types/api'
import { apiGet, apiPut } from '../utils/api'
import './ConfigView.css'

export function ConfigView() {
  const [serverConfig, setServerConfig] = useState<ServerConfig | null>(null)
  const [worldConfig, setWorldConfig] = useState<WorldConfig | null>(null)
  const [serverDirty, setServerDirty] = useState(false)
  const [worldDirty, setWorldDirty] = useState(false)
  const [saving, setSaving] = useState<'server' | 'world' | null>(null)
  const [status, setStatus] = useState<{ kind: 'ok' | 'err'; msg: string } | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    async function load() {
      setLoading(true)
      try {
        const [sRes, wRes] = await Promise.all([
          apiGet<ServerConfig>('/api/config/server'),
          apiGet<WorldConfig>('/api/config/world'),
        ])
        if (sRes.success && sRes.data) setServerConfig(sRes.data)
        if (wRes.success && wRes.data) setWorldConfig(wRes.data)
      } catch (e) {
        setStatus({ kind: 'err', msg: `Failed to load config: ${e instanceof Error ? e.message : e}` })
      } finally {
        setLoading(false)
      }
    }
    load()
  }, [])

  function updateServer<K extends keyof ServerConfig>(key: K, value: ServerConfig[K]) {
    setServerConfig((prev) => (prev ? { ...prev, [key]: value } : prev))
    setServerDirty(true)
  }

  function updateWorld<K extends keyof WorldConfig>(key: K, value: WorldConfig[K]) {
    setWorldConfig((prev) => (prev ? { ...prev, [key]: value } : prev))
    setWorldDirty(true)
  }

  async function saveServer() {
    if (!serverConfig) return
    setSaving('server')
    setStatus(null)
    try {
      const res = await apiPut<ServerConfig>('/api/config/server', serverConfig)
      if (res.success) {
        setServerDirty(false)
        setStatus({ kind: 'ok', msg: 'Server config saved.' })
      } else {
        setStatus({ kind: 'err', msg: res.message || 'Failed to save.' })
      }
    } catch (e) {
      setStatus({ kind: 'err', msg: `Error: ${e instanceof Error ? e.message : e}` })
    } finally {
      setSaving(null)
    }
  }

  async function saveWorld() {
    if (!worldConfig) return
    setSaving('world')
    setStatus(null)
    try {
      const res = await apiPut<WorldConfig>('/api/config/world', worldConfig)
      if (res.success) {
        setWorldDirty(false)
        setStatus({ kind: 'ok', msg: 'World config saved.' })
      } else {
        setStatus({ kind: 'err', msg: res.message || 'Failed to save.' })
      }
    } catch (e) {
      setStatus({ kind: 'err', msg: `Error: ${e instanceof Error ? e.message : e}` })
    } finally {
      setSaving(null)
    }
  }

  if (loading) {
    return <div className="config-view animate-fade-in"><p className="text-muted">Loading configuration…</p></div>
  }

  return (
    <div className="config-view animate-fade-in">
      {status && (
        <div className={`config-status ${status.kind === 'ok' ? 'config-status--ok' : 'config-status--err'}`}>
          {status.msg}
        </div>
      )}

      <div className="config-grid">
        {/* Server Config */}
        <div className="card config-panel">
          <div className="panel-title">
            <span className="panel-title-icon">🖥</span>
            Server Configuration
            {serverDirty && <span className="dirty-badge">unsaved</span>}
          </div>

          {serverConfig ? (
            <>
              <div className="field-group">
                <label className="field-label">Server Name</label>
                <input
                  className="input"
                  type="text"
                  value={serverConfig.server_name}
                  onChange={(e) => updateServer('server_name', e.target.value)}
                />
              </div>
              <div className="field-row">
                <div className="field-group">
                  <label className="field-label">Port</label>
                  <input
                    className="input"
                    type="number"
                    min={1}
                    max={65535}
                    value={serverConfig.port}
                    onChange={(e) => updateServer('port', parseInt(e.target.value, 10))}
                  />
                </div>
                <div className="field-group">
                  <label className="field-label">Max Players</label>
                  <input
                    className="input"
                    type="number"
                    min={1}
                    value={serverConfig.max_players}
                    onChange={(e) => updateServer('max_players', parseInt(e.target.value, 10))}
                  />
                </div>
              </div>
              <div className="field-group">
                <label className="field-label">Invite Code</label>
                <input
                  className="input"
                  type="text"
                  value={serverConfig.invite_code ?? ''}
                  onChange={(e) =>
                    updateServer('invite_code', e.target.value || null)
                  }
                  placeholder="Optional"
                />
              </div>

              <div className="config-actions">
                <button
                  className={`btn ${serverDirty ? 'btn-primary' : ''}`}
                  onClick={saveServer}
                  disabled={!serverDirty || saving === 'server'}
                >
                  {saving === 'server' ? 'Saving…' : 'Save Server Config'}
                </button>
              </div>
            </>
          ) : (
            <p className="text-faint" style={{ fontSize: '0.85rem' }}>
              No server config file found. Configure{' '}
              <code className="font-mono">server_working_dir</code> in AppConfig to enable.
            </p>
          )}
        </div>

        {/* World Config */}
        <div className="card config-panel">
          <div className="panel-title">
            <span className="panel-title-icon">🌊</span>
            World Configuration
            {worldDirty && <span className="dirty-badge">unsaved</span>}
          </div>

          {worldConfig ? (
            <>
              <div className="field-group">
                <label className="field-label">World Name</label>
                <input
                  className="input"
                  type="text"
                  value={worldConfig.world_name}
                  onChange={(e) => updateWorld('world_name', e.target.value)}
                />
              </div>
              <div className="field-group">
                <label className="field-label">Seed</label>
                <input
                  className="input"
                  type="text"
                  value={worldConfig.seed ?? ''}
                  onChange={(e) => updateWorld('seed', e.target.value || null)}
                  placeholder="Optional"
                />
              </div>

              <div className="config-actions">
                <button
                  className={`btn ${worldDirty ? 'btn-primary' : ''}`}
                  onClick={saveWorld}
                  disabled={!worldDirty || saving === 'world'}
                >
                  {saving === 'world' ? 'Saving…' : 'Save World Config'}
                </button>
              </div>
            </>
          ) : (
            <p className="text-faint" style={{ fontSize: '0.85rem' }}>
              No world config file found. Configure{' '}
              <code className="font-mono">server_working_dir</code> in AppConfig to enable.
            </p>
          )}
        </div>
      </div>
    </div>
  )
}
