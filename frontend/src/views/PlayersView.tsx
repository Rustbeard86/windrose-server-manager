import { useEffect, useState } from 'react'
import type { AppStateSnapshot, PlayerEvent } from '../types/api'
import { formatDateTime, formatTimestamp } from '../utils/format'
import './PlayersView.css'

interface PlayersViewProps {
  state: AppStateSnapshot
  playerEvents: PlayerEvent[]
}

export function PlayersView({ state, playerEvents }: PlayersViewProps) {
  const { players, player_count } = state
  const allEvents = [...playerEvents].reverse()

  // Tick every second so session timers stay live.
  const [now, setNow] = useState(() => Date.now())
  useEffect(() => {
    const id = setInterval(() => setNow(Date.now()), 1000)
    return () => clearInterval(id)
  }, [])

  return (
    <div className="players-view animate-fade-in">
      {/* Online players */}
      <div className="card players-card">
        <div className="panel-title">
          <span className="panel-title-icon">🟢</span>
          Online Players
          <span className="badge badge-running" style={{ marginLeft: 'auto' }}>{player_count}</span>
        </div>
        {players.length === 0 ? (
          <p className="text-faint" style={{ fontSize: '0.85rem' }}>No players currently online.</p>
        ) : (
          <table className="player-table">
            <thead>
              <tr>
                <th>Name</th>
                <th>Joined At</th>
                <th>Session</th>
              </tr>
            </thead>
            <tbody>
              {players.map((p) => {
                const joinedMs = new Date(p.joined_at).getTime()
                const sessionSec = Math.floor((now - joinedMs) / 1000)
                const hh = Math.floor(sessionSec / 3600)
                const mm = Math.floor((sessionSec % 3600) / 60)
                const ss = sessionSec % 60
                const session = [hh, mm, ss].map((n) => String(n).padStart(2, '0')).join(':')
                return (
                  <tr key={p.name}>
                    <td className="player-tbl-name">
                      <span className="dot dot-running" />
                      {p.name}
                    </td>
                    <td className="text-muted">{formatDateTime(p.joined_at)}</td>
                    <td className="font-mono text-teal">{session}</td>
                  </tr>
                )
              })}
            </tbody>
          </table>
        )}
      </div>

      {/* Event history */}
      <div className="card players-card">
        <div className="panel-title">
          <span className="panel-title-icon">📜</span>
          Activity History
          <span className="text-faint" style={{ fontSize: '0.72rem', marginLeft: 'auto' }}>
            {playerEvents.length} events
          </span>
        </div>
        <div className="player-history">
          {allEvents.length === 0 ? (
            <p className="text-faint" style={{ fontSize: '0.85rem' }}>No player events recorded yet.</p>
          ) : (
            allEvents.map((ev, i) => (
              <div key={i} className="history-row animate-fade-in">
                <span
                  className={`history-kind-badge ${ev.kind === 'joined' ? 'badge-running' : 'badge-stopped'} badge`}
                >
                  {ev.kind}
                </span>
                <span className="history-name">{ev.player_name}</span>
                <span className="history-time text-faint">{formatTimestamp(ev.timestamp)}</span>
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  )
}
