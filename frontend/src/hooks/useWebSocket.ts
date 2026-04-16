import { useCallback, useEffect, useRef, useState } from 'react'
import type { WsEvent } from '../types/api'

export type WsStatus = 'connecting' | 'connected' | 'disconnected' | 'error'

interface UseWebSocketOptions {
  onEvent?: (event: WsEvent) => void
  enabled?: boolean
}

export function useWebSocket({ onEvent, enabled = true }: UseWebSocketOptions) {
  const [status, setStatus] = useState<WsStatus>('disconnected')
  const wsRef = useRef<WebSocket | null>(null)
  const reconnectTimer = useRef<ReturnType<typeof setTimeout> | null>(null)
  const onEventRef = useRef(onEvent)
  // Use a ref for the connect function so the onclose callback can always
  // reference the latest version without closuring over a stale copy.
  const connectRef = useRef<() => void>(() => {})
  onEventRef.current = onEvent

  const connect = useCallback(() => {
    if (wsRef.current) {
      wsRef.current.onclose = null
      wsRef.current.close()
    }

    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
    const ws = new WebSocket(`${protocol}//${window.location.host}/ws`)
    wsRef.current = ws
    setStatus('connecting')

    ws.onopen = () => {
      setStatus('connected')
    }

    ws.onmessage = (e: MessageEvent<string>) => {
      try {
        const event = JSON.parse(e.data) as WsEvent
        onEventRef.current?.(event)
      } catch {
        // ignore parse errors
      }
    }

    ws.onerror = () => {
      setStatus('error')
    }

    ws.onclose = () => {
      setStatus('disconnected')
      wsRef.current = null
      // Reconnect after 5 seconds via ref to avoid closure-over-stale issue.
      reconnectTimer.current = setTimeout(() => connectRef.current(), 5000)
    }
  }, [])

  // Keep the ref in sync with the memoised callback.
  connectRef.current = connect

  useEffect(() => {
    if (!enabled) {
      setStatus('disconnected')
      return
    }

    connect()
    return () => {
      if (reconnectTimer.current) clearTimeout(reconnectTimer.current)
      if (wsRef.current) {
        wsRef.current.onclose = null
        wsRef.current.close()
      }
    }
  }, [connect, enabled])

  return { status }
}
