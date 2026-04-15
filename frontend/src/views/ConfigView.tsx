import { useCallback, useEffect, useRef, useState } from 'react'
import type { ConfigFileInfo, ConfigFileContent, ConfigFileKind } from '../types/api'
import { apiGet } from '../utils/api'
import './ConfigView.css'

// ──────────────────────────────────────────────────────────────────────────────
// Field spec helpers
// ──────────────────────────────────────────────────────────────────────────────

const WORLD_DESC_READONLY = new Set([
  'Version', 'IslandId', 'CreationTime',
])

const WORLD_PRESET_OPTIONS = ['Easy', 'Medium', 'Hard', 'Custom']

interface RangeSpec { min: number; max: number; step?: number }

const WORLD_SETTING_RANGES: Record<string, RangeSpec> = {
  MobHealthMultiplier:            { min: 0.2, max: 5.0, step: 0.1 },
  MobDamageMultiplier:            { min: 0.2, max: 5.0, step: 0.1 },
  ShipHealthMultiplier:           { min: 0.4, max: 5.0, step: 0.1 },
  ShipDamageMultiplier:           { min: 0.2, max: 2.5, step: 0.1 },
  BoardingDifficultyMultiplier:   { min: 0.2, max: 5.0, step: 0.1 },
  Coop_StatsCorrectionModifier:   { min: 0.0, max: 2.0, step: 0.1 },
  Coop_ShipStatsCorrectionModifier: { min: 0.0, max: 2.0, step: 0.1 },
}

const WORLD_SETTING_BOOLS = new Set(['CoopQuests', 'EasyExplore'])
const COMBAT_DIFFICULTY_OPTIONS = ['Easy', 'Normal', 'Hard']

type Tab = 'structured' | 'raw'

// ──────────────────────────────────────────────────────────────────────────────
// Component
// ──────────────────────────────────────────────────────────────────────────────

export function ConfigView() {
  // ── File list ──
  const [files, setFiles] = useState<ConfigFileInfo[]>([])
  const [selectedPath, setSelectedPath] = useState<string | null>(null)
  const [selectedKind, setSelectedKind] = useState<ConfigFileKind | null>(null)

  // ── Active file content ──
  const [content, setContent] = useState<string>('')
  const [parsed, setParsed] = useState<Record<string, unknown> | null>(null)
  const [lastModified, setLastModified] = useState<string>('')
  const [loading, setLoading] = useState(false)

  // ── Tabs ──
  const [tab, setTab] = useState<Tab>('structured')

  // ── Dirty tracking ──
  const [dirty, setDirty] = useState(false)
  const [originalContent, setOriginalContent] = useState<string>('')

  // ── Raw editor validation ──
  const [rawContent, setRawContent] = useState<string>('')
  const [rawError, setRawError] = useState<string | null>(null)

  // ── Save state ──
  const [saving, setSaving] = useState(false)
  const [status, setStatus] = useState<{ kind: 'ok' | 'err'; msg: string } | null>(null)

  // ── Conflict detection ──
  const [conflict, setConflict] = useState(false)
  const [diskContent, setDiskContent] = useState<string | null>(null)
  const [showDiff, setShowDiff] = useState(false)

  // ── Polling refs ──
  const fileListTimerRef = useRef<ReturnType<typeof setInterval>>(undefined)
  const mtimeTimerRef = useRef<ReturnType<typeof setInterval>>(undefined)

  // ── Fetch file list ──
  const fetchFiles = useCallback(async () => {
    try {
      const res = await apiGet<ConfigFileInfo[]>('/api/config/files')
      if (res.success && res.data) setFiles(res.data)
    } catch { /* ignore polling errors */ }
  }, [])

  // ── Fetch a single file ──
  const fetchFile = useCallback(async (path: string) => {
    setLoading(true)
    setStatus(null)
    setConflict(false)
    setDiskContent(null)
    setShowDiff(false)
    try {
      const res = await apiGet<ConfigFileContent>(`/api/config/file?path=${encodeURIComponent(path)}`)
      if (res.success && res.data) {
        const raw = res.data.content
        setContent(raw)
        setOriginalContent(raw)
        setRawContent(raw)
        setRawError(null)
        setLastModified(res.data.last_modified)
        setDirty(false)
        try {
          setParsed(JSON.parse(raw))
        } catch {
          setParsed(null)
        }
      }
    } catch (e) {
      setStatus({ kind: 'err', msg: `Failed to load file: ${e instanceof Error ? e.message : e}` })
    } finally {
      setLoading(false)
    }
  }, [])

  // ── File list polling (5s) ──
  useEffect(() => {
    fetchFiles()
    fileListTimerRef.current = setInterval(fetchFiles, 5000)
    return () => clearInterval(fileListTimerRef.current)
  }, [fetchFiles])

  // ── Auto-select first file ──
  useEffect(() => {
    if (files.length > 0 && !selectedPath) {
      setSelectedPath(files[0].path)
      setSelectedKind(files[0].kind)
    }
  }, [files, selectedPath])

  // ── Load file on selection ──
  useEffect(() => {
    if (selectedPath) fetchFile(selectedPath)
  }, [selectedPath, fetchFile])

  // ── Mtime polling (3s) for conflict detection ──
  useEffect(() => {
    if (!selectedPath || !lastModified) return
    const checkMtime = async () => {
      try {
        // The mtime endpoint returns ApiResponse<DateTime> — data is the raw datetime string
        const res = await apiGet<string>(`/api/config/file/mtime?path=${encodeURIComponent(selectedPath)}`)
        if (res.success && res.data && res.data !== lastModified) {
          if (dirty) {
            setConflict(true)
            const fileRes = await apiGet<ConfigFileContent>(`/api/config/file?path=${encodeURIComponent(selectedPath)}`)
            if (fileRes.success && fileRes.data) {
              setDiskContent(fileRes.data.content)
            }
          } else {
            // No unsaved edits — silently update content in-place (no loading flash)
            const fileRes = await apiGet<ConfigFileContent>(`/api/config/file?path=${encodeURIComponent(selectedPath)}`)
            if (fileRes.success && fileRes.data) {
              setContent(fileRes.data.content)
              setOriginalContent(fileRes.data.content)
              setRawContent(fileRes.data.content)
              setRawError(null)
              setLastModified(fileRes.data.last_modified)
              try { setParsed(JSON.parse(fileRes.data.content)) } catch { setParsed(null) }
            }
          }
        }
      } catch { /* ignore */ }
    }
    mtimeTimerRef.current = setInterval(checkMtime, 3000)
    return () => clearInterval(mtimeTimerRef.current)
  }, [selectedPath, lastModified, dirty])

  // ── Handlers ──

  function handleFileSelect(path: string) {
    const f = files.find(fi => fi.path === path)
    setSelectedPath(path)
    setSelectedKind(f?.kind ?? null)
    setDirty(false)
    setConflict(false)
    setShowDiff(false)
    setTab('structured')
  }

  function handleStructuredChange(key: string, value: unknown, nestedKey?: string) {
    if (!parsed) return
    let next: Record<string, unknown>
    if (nestedKey) {
      const nested = (parsed[nestedKey] ?? {}) as Record<string, unknown>
      next = { ...parsed, [nestedKey]: { ...nested, [key]: value } }
    } else {
      next = { ...parsed, [key]: value }
    }
    setParsed(next)
    const json = JSON.stringify(next, null, 2)
    setContent(json)
    setRawContent(json)
    setRawError(null)
    setDirty(true)
  }

  function handleWorldSettingChange(settingsKey: string, value: unknown) {
    if (!parsed) return
    const settings = (parsed.WorldSettings ?? {}) as Record<string, unknown>
    const nextSettings = { ...settings, [settingsKey]: value }
    handleStructuredChange('WorldSettings', nextSettings)
  }

  function handleRawChange(value: string) {
    setRawContent(value)
    setDirty(true)
    try {
      const obj = JSON.parse(value)
      setRawError(null)
      setParsed(obj)
      setContent(value)
    } catch (e) {
      setRawError(e instanceof Error ? e.message : 'Invalid JSON')
    }
  }

  async function handleSave() {
    if (!selectedPath) return
    const jsonToSave = tab === 'raw' ? rawContent : content
    setSaving(true)
    setStatus(null)
    try {
      const res = await fetch(`/api/config/file`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ path: selectedPath, content: jsonToSave, last_modified: lastModified }),
      })
      if (res.status === 409) {
        const body = await res.json()
        setConflict(true)
        if (body.data?.disk_content) setDiskContent(body.data.disk_content)
        if (body.data?.disk_mtime) setLastModified(body.data.disk_mtime)
        setStatus({ kind: 'err', msg: 'File was modified on disk. Resolve the conflict first.' })
      } else if (res.ok) {
        const body = await res.json()
        if (body.success) {
          setDirty(false)
          setOriginalContent(jsonToSave)
          setLastModified(body.data?.last_modified ?? lastModified)
          setConflict(false)
          setShowDiff(false)
          setDiskContent(null)
          setStatus({ kind: 'ok', msg: 'Saved.' })
        } else {
          setStatus({ kind: 'err', msg: body.message || 'Save failed.' })
        }
      } else {
        setStatus({ kind: 'err', msg: `HTTP ${res.status}` })
      }
    } catch (e) {
      setStatus({ kind: 'err', msg: `Error: ${e instanceof Error ? e.message : e}` })
    } finally {
      setSaving(false)
    }
  }

  function handleReloadFromDisk() {
    if (selectedPath) fetchFile(selectedPath)
  }

  // ──────────────────────────────────────────────────────────────────────
  // Render helpers
  // ──────────────────────────────────────────────────────────────────────

  function renderFilePicker() {
    return (
      <div className="config-file-picker">
        <label className="field-label" style={{ marginBottom: 2 }}>Config File</label>
        <div className="picker-row">
          <select
            className="input config-select"
            value={selectedPath ?? ''}
            onChange={(e) => handleFileSelect(e.target.value)}
          >
            {files.length === 0 && <option value="">No config files found</option>}
            {files.map(f => (
              <option key={f.path} value={f.path}>
                {f.kind === 'server_description'
                  ? `Server — ${f.file_name}`
                  : `World — ${f.path}`}
              </option>
            ))}
          </select>
          <button className="btn btn-sm" onClick={fetchFiles} title="Refresh file list">↻</button>
        </div>
      </div>
    )
  }

  function renderConflictBanner() {
    if (!conflict) return null
    return (
      <div className="conflict-banner">
        <span className="conflict-icon">⚠</span>
        <span className="conflict-text">This file was modified on disk.</span>
        <button className="btn btn-sm" onClick={handleReloadFromDisk}>Reload from disk</button>
        <button className="btn btn-sm" onClick={() => setShowDiff(!showDiff)}>
          {showDiff ? 'Hide diff' : 'Show diff'}
        </button>
      </div>
    )
  }

  function renderDiffView() {
    if (!showDiff || !diskContent) return null
    return (
      <div className="diff-container">
        <div className="diff-panel">
          <div className="diff-panel-header">Your edits</div>
          <pre className="diff-content">{tab === 'raw' ? rawContent : content}</pre>
        </div>
        <div className="diff-panel">
          <div className="diff-panel-header">Disk version</div>
          <pre className="diff-content">{diskContent}</pre>
        </div>
      </div>
    )
  }

  function renderReadOnlyField(label: string, value: unknown) {
    return (
      <div className="field-group" key={label}>
        <label className="field-label">
          {label} <span className="readonly-badge">read-only</span>
        </label>
        <input className="input input--readonly" type="text" value={String(value ?? '')} readOnly />
      </div>
    )
  }

  function renderServerDescFields() {
    if (!parsed) return <p className="text-faint">Could not parse JSON.</p>
    const fields: React.ReactNode[] = []

    // ServerDescription.json nests editable fields inside ServerDescription_Persistent
    const persistent = (parsed.ServerDescription_Persistent ?? parsed) as Record<string, unknown>
    const nestKey = 'ServerDescription_Persistent' in parsed ? 'ServerDescription_Persistent' : undefined

    // Top-level read-only fields (Version, DeploymentId)
    for (const key of ['Version', 'DeploymentId']) {
      if (key in parsed) fields.push(renderReadOnlyField(key, parsed[key]))
    }

    // Nested read-only fields (PersistentServerId, WorldIslandId)
    for (const key of ['PersistentServerId', 'WorldIslandId']) {
      if (key in persistent) fields.push(renderReadOnlyField(key, persistent[key]))
    }

    // ServerName
    fields.push(
      <div className="field-group" key="ServerName">
        <label className="field-label">ServerName</label>
        <input
          className="input"
          type="text"
          value={String(persistent.ServerName ?? '')}
          onChange={(e) => handleStructuredChange('ServerName', e.target.value, nestKey)}
        />
      </div>
    )

    // InviteCode
    const inviteCode = String(persistent.InviteCode ?? '')
    const inviteErr = inviteCode.length > 0 && (inviteCode.length < 6 || !/^[A-Za-z0-9]+$/.test(inviteCode))
    fields.push(
      <div className="field-group" key="InviteCode">
        <label className="field-label">InviteCode</label>
        <input
          className={`input ${inviteErr ? 'input--invalid' : ''}`}
          type="text"
          value={inviteCode}
          onChange={(e) => handleStructuredChange('InviteCode', e.target.value, nestKey)}
        />
        {inviteErr && <span className="field-error">Min 6 characters, alphanumeric only</span>}
      </div>
    )

    // MaxPlayerCount
    fields.push(
      <div className="field-group" key="MaxPlayerCount">
        <label className="field-label">MaxPlayerCount</label>
        <input
          className="input"
          type="number"
          min={1}
          value={Number(persistent.MaxPlayerCount ?? 1)}
          onChange={(e) => handleStructuredChange('MaxPlayerCount', Math.max(1, parseInt(e.target.value, 10) || 1), nestKey)}
        />
      </div>
    )

    // IsPasswordProtected + Password
    const isPwProtected = Boolean(persistent.IsPasswordProtected)
    fields.push(
      <div className="field-group" key="IsPasswordProtected">
        <label className="field-label">
          <input
            type="checkbox"
            checked={isPwProtected}
            onChange={(e) => handleStructuredChange('IsPasswordProtected', e.target.checked, nestKey)}
          />{' '}
          Password Protected
        </label>
      </div>
    )
    if (isPwProtected) {
      fields.push(
        <div className="field-group" key="Password">
          <label className="field-label">Password</label>
          <input
            className="input"
            type="text"
            value={String(persistent.Password ?? '')}
            onChange={(e) => handleStructuredChange('Password', e.target.value, nestKey)}
          />
        </div>
      )
    }

    // P2pProxyAddress
    if ('P2pProxyAddress' in persistent) {
      fields.push(
        <div className="field-group" key="P2pProxyAddress">
          <label className="field-label">P2pProxyAddress</label>
          <input
            className="input"
            type="text"
            value={String(persistent.P2pProxyAddress ?? '')}
            onChange={(e) => handleStructuredChange('P2pProxyAddress', e.target.value, nestKey)}
            placeholder="IP address"
          />
        </div>
      )
    }

    return <>{fields}</>
  }

  function renderWorldDescFields() {
    if (!parsed) return <p className="text-faint">Could not parse JSON.</p>
    const fields: React.ReactNode[] = []

    // Read-only fields
    for (const key of WORLD_DESC_READONLY) {
      if (key in parsed) fields.push(renderReadOnlyField(key, parsed[key]))
    }

    // WorldName
    fields.push(
      <div className="field-group" key="WorldName">
        <label className="field-label">WorldName</label>
        <input
          className="input"
          type="text"
          value={String(parsed.WorldName ?? '')}
          onChange={(e) => handleStructuredChange('WorldName', e.target.value)}
        />
      </div>
    )

    // WorldPresetType
    const preset = String(parsed.WorldPresetType ?? 'Custom')
    fields.push(
      <div className="field-group" key="WorldPresetType">
        <label className="field-label">WorldPresetType</label>
        <select
          className="input"
          value={preset}
          onChange={(e) => handleStructuredChange('WorldPresetType', e.target.value)}
        >
          {WORLD_PRESET_OPTIONS.map(o => <option key={o} value={o}>{o}</option>)}
        </select>
      </div>
    )

    // WorldSettings (shown only when Custom, or always if present)
    const settings = (parsed.WorldSettings ?? {}) as Record<string, unknown>
    if (preset === 'Custom' || Object.keys(settings).length > 0) {
      fields.push(
        <div className="field-section" key="WorldSettings">
          <div className="field-section-title">World Settings</div>

          {/* Bool params */}
          {[...WORLD_SETTING_BOOLS].map(key => (
            <div className="field-group" key={key}>
              <label className="field-label">
                <input
                  type="checkbox"
                  checked={Boolean(settings[key])}
                  onChange={(e) => handleWorldSettingChange(key, e.target.checked)}
                />{' '}
                {key}
              </label>
            </div>
          ))}

          {/* Float params */}
          {Object.entries(WORLD_SETTING_RANGES).map(([key, range]) => {
            const val = Number(settings[key] ?? range.min)
            return (
              <div className="field-group" key={key}>
                <label className="field-label">
                  {key} <span className="field-range">({range.min} – {range.max})</span>
                </label>
                <input
                  className="input"
                  type="number"
                  min={range.min}
                  max={range.max}
                  step={range.step ?? 0.1}
                  value={val}
                  onChange={(e) => {
                    const n = parseFloat(e.target.value)
                    if (!isNaN(n)) handleWorldSettingChange(key, Math.min(range.max, Math.max(range.min, n)))
                  }}
                />
              </div>
            )
          })}

          {/* CombatDifficulty tag */}
          <div className="field-group" key="CombatDifficulty">
            <label className="field-label">CombatDifficulty</label>
            <select
              className="input"
              value={String(settings.CombatDifficulty ?? 'Normal')}
              onChange={(e) => handleWorldSettingChange('CombatDifficulty', e.target.value)}
            >
              {COMBAT_DIFFICULTY_OPTIONS.map(o => <option key={o} value={o}>{o}</option>)}
            </select>
          </div>
        </div>
      )
    }

    return <>{fields}</>
  }

  function renderStructuredTab() {
    if (!parsed) return <p className="text-faint">Could not parse file as JSON.</p>
    const kind = selectedKind
    if (kind === 'server_description') return renderServerDescFields()
    if (kind === 'world_description') return renderWorldDescFields()
    return <p className="text-muted">Unknown file type. Use the Raw JSON tab to edit.</p>
  }

  function renderRawTab() {
    return (
      <div className="raw-editor">
        <textarea
          className={`input raw-textarea ${rawError ? 'input--invalid' : rawContent !== originalContent ? 'input--modified' : ''}`}
          value={rawContent}
          onChange={(e) => handleRawChange(e.target.value)}
          spellCheck={false}
        />
        {rawError && <div className="raw-error">{rawError}</div>}
      </div>
    )
  }

  // ──────────────────────────────────────────────────────────────────────
  // Main render
  // ──────────────────────────────────────────────────────────────────────

  return (
    <div className="config-view animate-fade-in">
      {renderFilePicker()}

      {status && (
        <div className={`config-status ${status.kind === 'ok' ? 'config-status--ok' : 'config-status--err'}`}>
          {status.msg}
        </div>
      )}

      {renderConflictBanner()}
      {renderDiffView()}

      {loading ? (
        <p className="text-muted">Loading file…</p>
      ) : selectedPath ? (
        <>
          <div className="config-tabs">
            <button
              className={`tab-btn ${tab === 'structured' ? 'tab-btn--active' : ''}`}
              onClick={() => setTab('structured')}
            >
              Structured
            </button>
            <button
              className={`tab-btn ${tab === 'raw' ? 'tab-btn--active' : ''}`}
              onClick={() => setTab('raw')}
            >
              Raw JSON
            </button>
            {dirty && <span className="dirty-badge">unsaved</span>}
          </div>

          <div className="card config-panel">
            {tab === 'structured' ? renderStructuredTab() : renderRawTab()}

            <div className="config-actions">
              <button
                className={`btn ${dirty ? 'btn-primary' : ''}`}
                onClick={handleSave}
                disabled={!dirty || saving || (tab === 'raw' && !!rawError)}
              >
                {saving ? 'Saving…' : 'Save'}
              </button>
              {dirty && (
                <button className="btn" onClick={handleReloadFromDisk}>
                  Discard changes
                </button>
              )}
            </div>
          </div>
        </>
      ) : (
        <div className="card config-panel">
          <p className="text-faint" style={{ fontSize: '0.85rem' }}>
            No config files found. Ensure <strong>server_working_dir</strong> is configured correctly in the Setup Wizard.
          </p>
        </div>
      )}
    </div>
  )
}
