import type { WsStatus } from '../hooks/useWebSocket'
import type { ConnectionStatus } from '../hooks/useAppState'
import './AppHeader.css'

interface AppHeaderProps {
  wsStatus: WsStatus
  connectionStatus: ConnectionStatus
  appVersion?: string
  activeView: string
  onViewChange: (view: string) => void
}

const NAV_ITEMS = [
  { id: 'dashboard', label: 'Dashboard', icon: '⚓' },
  { id: 'logs', label: 'Logs', icon: '📋' },
  { id: 'players', label: 'Players', icon: '👥' },
  { id: 'config', label: 'Config', icon: '⚙️' },
  { id: 'operations', label: 'Operations', icon: '🔧' },
]

export function AppHeader({
  wsStatus,
  connectionStatus,
  appVersion,
  activeView,
  onViewChange,
}: AppHeaderProps) {
  const isOnline = wsStatus === 'connected' && connectionStatus === 'connected'

  return (
    <header className="app-header">
      {/* Logo */}
      <div className="header-logo">
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
        <div className="logo-text">
          <span className="logo-title">Windrose</span>
          <span className="logo-subtitle">Server Manager</span>
        </div>
      </div>

      {/* Navigation */}
      <nav className="header-nav" role="navigation">
        {NAV_ITEMS.map((item) => (
          <button
            key={item.id}
            className={`nav-item ${activeView === item.id ? 'nav-item--active' : ''}`}
            onClick={() => onViewChange(item.id)}
          >
            <span className="nav-icon">{item.icon}</span>
            <span className="nav-label">{item.label}</span>
          </button>
        ))}
      </nav>

      {/* Status pill */}
      <div className={`connection-pill ${isOnline ? 'connection-pill--online' : 'connection-pill--offline'}`}>
        <span className={`dot ${isOnline ? 'dot-ws' : 'dot-stopped'}`} />
        <span className="connection-label">
          {wsStatus === 'connecting' ? 'Connecting…' : isOnline ? 'Backend Online' : 'Reconnecting…'}
        </span>
        {appVersion && (
          <span className="connection-version">v{appVersion}</span>
        )}
      </div>
    </header>
  )
}
