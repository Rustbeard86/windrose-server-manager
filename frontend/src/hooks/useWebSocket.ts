import { useEffect, useRef, useState } from 'react'
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
  onEventRef.current = onEvent

  // Connect logic - not memoized to avoid dependency issues
  const connect = () => {
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
      // Reconnect after 5 seconds
      reconnectTimer.current = setTimeout(() => connect(), 5000)
    }
  }

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
  }, [enabled])

  return { status }
}
