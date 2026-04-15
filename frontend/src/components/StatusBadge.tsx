import type { ServerStatus } from '../types/api'
import './StatusBadge.css'

interface StatusBadgeProps {
  status: ServerStatus
  showDot?: boolean
}

const LABELS: Record<ServerStatus, string> = {
  stopped: 'Stopped',
  starting: 'Starting',
  running: 'Running',
  stopping: 'Stopping',
  crashed: 'Crashed',
}

export function StatusBadge({ status, showDot = true }: StatusBadgeProps) {
  return (
    <span className={`badge badge-${status}`}>
      {showDot && <span className={`dot dot-${status}`} />}
      {LABELS[status]}
    </span>
  )
}
