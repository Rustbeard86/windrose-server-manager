import { useCallback, useEffect, useRef, useState } from 'react'
import type {
  AppStateSnapshot,
  BackupEntry,
  LogLine,
  PlayerEvent,
  WsEvent,
} from '../types/api'
import { apiGet } from '../utils/api'
import { useWebSocket } from './useWebSocket'

const MAX_LOG_LINES = 500

export type ConnectionStatus = 'loading' | 'connected' | 'disconnected' | 'error'

export function useAppState() {
  const [state, setState] = useState<AppStateSnapshot | null>(null)
  const [connectionStatus, setConnectionStatus] = useState<ConnectionStatus>('loading')
  const [liveLogLines, setLiveLogLines] = useState<LogLine[]>([])
  const [livePlayerEvents, setLivePlayerEvents] = useState<PlayerEvent[]>([])

  const loadState = useCallback(async () => {
    try {
      const response = await apiGet<AppStateSnapshot>('/api/state')
      if (response.success && response.data) {
        setState(response.data)
        setLiveLogLines(response.data.recent_logs)
        setLivePlayerEvents(response.data.player_events)
        setConnectionStatus('connected')
      }
    } catch {
      setConnectionStatus('error')
    }
  }, [])

  const handleWsEvent = useCallback((event: WsEvent) => {
    switch (event.event) {
      case 'server_status_changed':
        setState((prev) =>
          prev ? { ...prev, server: event.data } : prev
        )
        break
      case 'log_line':
        setLiveLogLines((prev) => {
          const next = [...prev, event.data]
          return next.length > MAX_LOG_LINES ? next.slice(-MAX_LOG_LINES) : next
        })
        break
      case 'player_joined':
        setState((prev) => {
          if (!prev) return prev
          const alreadyOnline = prev.players.some(
            (p) => p.name === event.data.player_name
          )
          const updatedPlayers = alreadyOnline
            ? prev.players
            : [
                ...prev.players,
                { name: event.data.player_name, joined_at: new Date().toISOString() },
              ]
          return {
            ...prev,
            players: updatedPlayers,
            player_count: updatedPlayers.length,
          }
        })
        setLivePlayerEvents((prev) => [
          ...prev,
          {
            player_name: event.data.player_name,
            kind: 'joined',
            timestamp: new Date().toISOString(),
          },
        ])
        break
      case 'player_left':
        setState((prev) => {
          if (!prev) return prev
          const updatedPlayers = prev.players.filter(
            (p) => p.name !== event.data.player_name
          )
          return {
            ...prev,
            players: updatedPlayers,
            player_count: updatedPlayers.length,
          }
        })
        setLivePlayerEvents((prev) => [
          ...prev,
          {
            player_name: event.data.player_name,
            kind: 'left',
            timestamp: new Date().toISOString(),
          },
        ])
        break
      case 'backup_progress':
        setState((prev) => {
          if (!prev) return prev
          const history: BackupEntry[] =
            event.data.entry &&
            event.data.job_state === 'done' &&
            !prev.backup.history.find((e) => e.id === event.data.entry!.id)
              ? [...prev.backup.history, event.data.entry!]
              : prev.backup.history
          return {
            ...prev,
            backup: {
              ...prev.backup,
              job_state: event.data.job_state as AppStateSnapshot['backup']['job_state'],
              progress_pct: event.data.progress_pct,
              current_file: event.data.current_file,
              history,
            },
          }
        })
        break
      case 'schedule_countdown':
        setState((prev) => {
          if (!prev) return prev
          return {
            ...prev,
            schedule: {
              ...prev.schedule,
              countdown_active: !event.data.cancelled && event.data.seconds_remaining > 0,
              countdown_seconds_remaining: event.data.cancelled
                ? null
                : event.data.seconds_remaining,
            },
          }
        })
        break
      case 'install_progress':
        setState((prev) => {
          if (!prev) return prev
          return {
            ...prev,
            install: {
              ...prev.install,
              job_state: event.data.job_state as AppStateSnapshot['install']['job_state'],
              progress_pct: event.data.progress_pct,
              current_file: event.data.current_file,
            },
          }
        })
        break
      case 'update_available':
        setState((prev) => {
          if (!prev) return prev
          return {
            ...prev,
            update: {
              ...prev.update,
              update_available: true,
              latest_version: event.data.latest_version,
              download_url: event.data.download_url,
            },
          }
        })
        break
      default:
        break
    }
  }, [])

  const { status: wsStatus } = useWebSocket({ onEvent: handleWsEvent })

  const prevWsRef = useRef(wsStatus)
  useEffect(() => {
    if (prevWsRef.current !== wsStatus) {
      if (wsStatus === 'connected') {
        // Re-hydrate state on reconnect (loadState is an async fetch helper,
        // setState is called in its async callback, not synchronously in the effect body).
        void loadState()
      }
      prevWsRef.current = wsStatus
    }
  }, [wsStatus, loadState])

  useEffect(() => {
    void loadState()
  }, [loadState])

  return {
    state,
    connectionStatus,
    wsStatus,
    liveLogLines,
    livePlayerEvents,
    reload: loadState,
  }
}
