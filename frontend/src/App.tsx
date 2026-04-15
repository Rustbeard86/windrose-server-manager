import { useEffect, useState } from 'react'
import { useAppState } from './hooks/useAppState'
import { AppHeader } from './components/AppHeader'
import { DashboardView } from './views/DashboardView'
import { LogsView } from './views/LogsView'
import { PlayersView } from './views/PlayersView'
import { ConfigView } from './views/ConfigView'
import { OperationsView } from './views/OperationsView'
import { SetupWizard } from './views/SetupWizard'
import type { SetupStatus } from './types/api'
import { apiGet } from './utils/api'
import './App.css'

type ViewId = 'dashboard' | 'logs' | 'players' | 'config' | 'operations'

export default function App() {
  const [activeView, setActiveView] = useState<ViewId>('dashboard')
  const [setupNeeded, setSetupNeeded] = useState<boolean | null>(null)
  const [forceSetup, setForceSetup] = useState(false)
  const { state, connectionStatus, wsStatus, liveLogLines, livePlayerEvents, reload } =
    useAppState()

  useEffect(() => {
    async function checkSetup() {
      try {
        const res = await apiGet<SetupStatus>('/api/setup/status')
        if (res.success && res.data) {
          setSetupNeeded(res.data.needs_setup)
        } else {
          setSetupNeeded(false)
        }
      } catch {
        setSetupNeeded(false)
      }
    }
    checkSetup()
  }, [])

  function handleSetupComplete() {
    setSetupNeeded(false)
    setForceSetup(false)
    reload()
  }

  function handleReRunSetup() {
    setForceSetup(true)
  }

  if (setupNeeded === null) {
    return (
      <div className="app-shell">
        <div className="loading-state">
          <p className="loading-msg">Checking configuration…</p>
        </div>
      </div>
    )
  }

  if (setupNeeded || forceSetup) {
    return <SetupWizard onComplete={handleSetupComplete} />
  }

  function renderView() {
    if (!state) {
      return (
        <div className="loading-state">
          <div className="loading-compass">
            <svg
              className="spin-slow"
              viewBox="0 0 40 40"
              fill="none"
              xmlns="http://www.w3.org/2000/svg"
              width="64"
              height="64"
            >
              <circle cx="20" cy="20" r="18" stroke="#c9a84c" strokeWidth="1.5" strokeDasharray="4 2" />
              <polygon points="20,4 22,20 20,22 18,20" fill="#c9a84c" />
              <polygon points="20,36 22,20 20,18 18,20" fill="#7a8fa6" />
              <polygon points="4,20 20,18 22,20 20,22" fill="#4ab8c8" />
              <polygon points="36,20 20,22 18,20 20,18" fill="#7a8fa6" />
              <circle cx="20" cy="20" r="2.5" fill="#c9a84c" />
            </svg>
          </div>
          <p className="loading-msg">
            {connectionStatus === 'error'
              ? 'Could not reach backend. Retrying…'
              : 'Connecting to Windrose backend…'}
          </p>
        </div>
      )
    }

    switch (activeView) {
      case 'dashboard':
        return <DashboardView state={state} onReload={reload} />
      case 'logs':
        return <LogsView logs={liveLogLines} />
      case 'players':
        return <PlayersView state={state} playerEvents={livePlayerEvents} />
      case 'config':
        return <ConfigView />
      case 'operations':
        return <OperationsView state={state} onReload={reload} />
      default:
        return <DashboardView state={state} onReload={reload} />
    }
  }

  return (
    <div className="app-shell">
      <AppHeader
        wsStatus={wsStatus}
        connectionStatus={connectionStatus}
        appVersion={state?.app_version}
        activeView={activeView}
        onViewChange={(v) => setActiveView(v as ViewId)}
        onReRunSetup={handleReRunSetup}
      />
      <main className="app-main">
        <div className="view-container">{renderView()}</div>
      </main>
      <footer className="app-footer">
        Windrose Server Manager
        {state?.app_version && <> &mdash; v{state.app_version}</>}
        {' '}&mdash; 127.0.0.1:8787
      </footer>
    </div>
  )
}
