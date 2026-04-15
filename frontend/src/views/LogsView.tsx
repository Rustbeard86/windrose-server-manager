import { useEffect, useRef, useState } from 'react'
import type { LogLine, LogLevel } from '../types/api'
import { formatTimestamp } from '../utils/format'
import './LogsView.css'

interface LogsViewProps {
  logs: LogLine[]
}

const LEVELS: LogLevel[] = ['INFO', 'WARN', 'ERROR', 'DEBUG', 'UNKNOWN']

export function LogsView({ logs }: LogsViewProps) {
  const [filter, setFilter] = useState('')
  const [levelFilter, setLevelFilter] = useState<LogLevel | 'ALL'>('ALL')
  const [autoScroll, setAutoScroll] = useState(true)
  const endRef = useRef<HTMLDivElement>(null)
  const containerRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (autoScroll && endRef.current) {
      endRef.current.scrollIntoView({ behavior: 'smooth' })
    }
  }, [logs, autoScroll])

  function handleScroll() {
    const el = containerRef.current
    if (!el) return
    const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 60
    setAutoScroll(atBottom)
  }

  const filtered = logs.filter((line) => {
    const matchLevel = levelFilter === 'ALL' || line.level === levelFilter
    const matchText =
      !filter ||
      line.raw.toLowerCase().includes(filter.toLowerCase()) ||
      line.message.toLowerCase().includes(filter.toLowerCase())
    return matchLevel && matchText
  })

  return (
    <div className="logs-view animate-fade-in">
      <div className="logs-toolbar card">
        <div className="panel-title" style={{ marginBottom: 0, paddingBottom: 0, border: 'none' }}>
          <span className="panel-title-icon">📋</span>
          Server Logs
          <span className="text-faint" style={{ fontSize: '0.72rem', marginLeft: 'auto' }}>
            {filtered.length} / {logs.length} lines
          </span>
        </div>
        <div className="logs-filters">
          <input
            className="input"
            type="search"
            placeholder="Filter logs…"
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
          />
          <select
            className="input"
            value={levelFilter}
            onChange={(e) => setLevelFilter(e.target.value as LogLevel | 'ALL')}
            style={{ maxWidth: '120px' }}
          >
            <option value="ALL">All levels</option>
            {LEVELS.map((l) => (
              <option key={l} value={l}>{l}</option>
            ))}
          </select>
          <button
            className={`btn btn-sm ${autoScroll ? 'btn-primary' : ''}`}
            onClick={() => {
              setAutoScroll((v) => !v)
              if (!autoScroll) endRef.current?.scrollIntoView({ behavior: 'smooth' })
            }}
            title="Toggle auto-scroll"
          >
            {autoScroll ? '⬇ Live' : '⏸ Paused'}
          </button>
        </div>
      </div>

      <div
        className="logs-output card"
        ref={containerRef}
        onScroll={handleScroll}
      >
        {filtered.length === 0 ? (
          <div className="logs-empty text-faint">No log lines {filter || levelFilter !== 'ALL' ? 'match current filter' : 'yet'}</div>
        ) : (
          filtered.map((line, i) => (
            <div key={i} className={`log-row log-${line.level}`}>
              <span className="log-ts">{formatTimestamp(line.timestamp)}</span>
              <span className={`log-level log-${line.level}`}>{line.level.padEnd(7)}</span>
              <span className="log-msg">{line.message || line.raw}</span>
            </div>
          ))
        )}
        <div ref={endRef} />
      </div>
    </div>
  )
}
