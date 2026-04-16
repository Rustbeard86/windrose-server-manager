import { useEffect, useState } from 'react'
import { useAppState } from './hooks/useAppState'
import { AppHeader } from './components/AppHeader'
import { DashboardView } from './views/DashboardView'
import { LogsView } from './views/LogsView'
import { PlayersView } from './views/PlayersView'
import { ConfigView } from './views/ConfigView'
import { OperationsView } from './views/OperationsView'
import { AuthAdminView } from './views/AuthAdminView'
import { SetupWizard } from './views/SetupWizard'
import type { AuthStatus, SessionInfo, SetupStatus } from './types/api'
import { apiGet, apiPost } from './utils/api'
import {
  hasPermission,
  PERM_MANAGE_BACKUPS,
  PERM_MANAGE_CONFIG,
  PERM_MANAGE_INSTALL,
  PERM_MANAGE_SCHEDULE,
  PERM_MANAGE_SERVER,
  PERM_MANAGE_UPDATES,
  PERM_MANAGE_USERS,
} from './utils/permissions'
import './App.css'

type ViewId = 'dashboard' | 'logs' | 'players' | 'config' | 'operations' | 'admin'

export default function App() {
  const [activeView, setActiveView] = useState<ViewId>('dashboard')
  const [setupNeeded, setSetupNeeded] = useState<boolean | null>(null)
  const [forceSetup, setForceSetup] = useState(false)
  const [authStatus, setAuthStatus] = useState<AuthStatus | null>(null)
  const [sessionInfo, setSessionInfo] = useState<SessionInfo | null>(null)
  const { state, connectionStatus, wsStatus, liveLogLines, livePlayerEvents, reload } =
    useAppState(setupNeeded === false && !!sessionInfo)

  useEffect(() => {
    async function bootstrapChecks() {
      try {
        const [setupRes, authRes] = await Promise.all([
          apiGet<SetupStatus>('/api/setup/status'),
          apiGet<AuthStatus>('/api/auth/status'),
        ])

        setSetupNeeded(setupRes.success && setupRes.data ? setupRes.data.needs_setup : false)
        setAuthStatus(authRes.success && authRes.data ? authRes.data : { has_users: false, needs_bootstrap: true })

        if (authRes.success && authRes.data?.has_users) {
          try {
            const me = await apiGet<SessionInfo>('/api/auth/me')
            if (me.success && me.data) {
              setSessionInfo(me.data)
            } else {
              setSessionInfo(null)
            }
          } catch {
            setSessionInfo(null)
          }
        } else {
          setSessionInfo(null)
        }
      } catch {
        setSetupNeeded(false)
        setAuthStatus({ has_users: false, needs_bootstrap: true })
        setSessionInfo(null)
      }
    }
    bootstrapChecks()
  }, [])

  function handleSetupComplete() {
    setSetupNeeded(false)
    setForceSetup(false)
    void refreshAuthSession()
  }

  function handleReRunSetup() {
    setForceSetup(true)
  }

  async function refreshAuthSession() {
    try {
      const authRes = await apiGet<AuthStatus>('/api/auth/status')
      setAuthStatus(authRes.success && authRes.data ? authRes.data : { has_users: false, needs_bootstrap: true })
      if (authRes.success && authRes.data?.has_users) {
        try {
          const me = await apiGet<SessionInfo>('/api/auth/me')
          setSessionInfo(me.success && me.data ? me.data : null)
        } catch {
          setSessionInfo(null)
        }
      } else {
        setSessionInfo(null)
      }
    } catch {
      setSessionInfo(null)
    }
  }

  async function handleLogout() {
    try {
      await apiPost('/api/auth/logout')
    } finally {
      setSessionInfo(null)
    }
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

  if (!authStatus) {
    return (
      <div className="app-shell">
        <div className="loading-state">
          <p className="loading-msg">Checking authentication…</p>
        </div>
      </div>
    )
  }

  if (authStatus.needs_bootstrap) {
    return <BootstrapAdminView onComplete={refreshAuthSession} />
  }

  if (authStatus.has_users && !sessionInfo) {
    return <LoginView onLoggedIn={refreshAuthSession} />
  }

  const canManageServer = hasPermission(sessionInfo, PERM_MANAGE_SERVER)
  const canManageConfig = hasPermission(sessionInfo, PERM_MANAGE_CONFIG)
  const canManageBackups = hasPermission(sessionInfo, PERM_MANAGE_BACKUPS)
  const canManageInstall = hasPermission(sessionInfo, PERM_MANAGE_INSTALL)
  const canManageUpdates = hasPermission(sessionInfo, PERM_MANAGE_UPDATES)
  const canManageSchedule = hasPermission(sessionInfo, PERM_MANAGE_SCHEDULE)
  const canManageUsers = hasPermission(sessionInfo, PERM_MANAGE_USERS)

  const navItems = [
    { id: 'dashboard', label: 'Dashboard', icon: '⚓' },
    { id: 'logs', label: 'Logs', icon: '📋' },
    { id: 'players', label: 'Players', icon: '👥' },
    ...(canManageConfig ? [{ id: 'config', label: 'Config', icon: '⚙️' }] : []),
    { id: 'operations', label: 'Operations', icon: '🔧' },
    ...(canManageUsers ? [{ id: 'admin', label: 'Admin', icon: '🛡️' }] : []),
  ]

  useEffect(() => {
    if (!navItems.some((v) => v.id === activeView)) {
      setActiveView('dashboard')
    }
  }, [activeView, navItems])

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
        return <DashboardView state={state} onReload={reload} canManageServer={canManageServer} />
      case 'logs':
        return <LogsView logs={liveLogLines} />
      case 'players':
        return <PlayersView state={state} playerEvents={livePlayerEvents} />
      case 'config':
        return <ConfigView />
      case 'operations':
        return (
          <OperationsView
            state={state}
            onReload={reload}
            canManageBackups={canManageBackups}
            canManageSchedule={canManageSchedule}
            canManageUpdates={canManageUpdates}
            canManageInstall={canManageInstall}
          />
        )
      case 'admin':
        return <AuthAdminView canManageUsers={canManageUsers} />
      default:
        return <DashboardView state={state} onReload={reload} canManageServer={canManageServer} />
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
        navItems={navItems}
        rightSlot={
          <button className="btn btn-sm" onClick={() => void handleLogout()}>
            Logout {sessionInfo?.username ? `(${sessionInfo.username})` : ''}
          </button>
        }
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

function BootstrapAdminView({ onComplete }: { onComplete: () => Promise<void> | void }) {
  const [username, setUsername] = useState('admin')
  const [password, setPassword] = useState('')
  const [confirmPassword, setConfirmPassword] = useState('')
  const [busy, setBusy] = useState(false)
  const [message, setMessage] = useState<string | null>(null)

  async function submit(e: React.FormEvent) {
    e.preventDefault()
    if (password.length < 10) {
      setMessage('Password must be at least 10 characters')
      return
    }
    if (password !== confirmPassword) {
      setMessage('Passwords do not match')
      return
    }
    setBusy(true)
    setMessage(null)
    try {
      const res = await apiPost('/api/auth/bootstrap', { username, password })
      if (res.success) {
        setMessage('Admin account created. You can now log in.')
        await onComplete()
      } else {
        setMessage(res.message ?? 'Bootstrap failed')
      }
    } catch (err) {
      setMessage(err instanceof Error ? err.message : 'Bootstrap failed')
    } finally {
      setBusy(false)
    }
  }

  return (
    <div className="app-shell">
      <main className="app-main auth-main">
        <section className="card auth-card">
          <h2 className="auth-title">Create Initial Admin Account</h2>
          <p className="text-faint auth-subtitle">
            No users exist yet. Create the first administrator to secure the dashboard.
          </p>
          <form className="auth-form" onSubmit={submit}>
            <label className="field-label">Username</label>
            <input className="input" value={username} onChange={(e) => setUsername(e.target.value)} required />

            <label className="field-label">Password</label>
            <input className="input" type="password" value={password} onChange={(e) => setPassword(e.target.value)} required />

            <label className="field-label">Confirm Password</label>
            <input className="input" type="password" value={confirmPassword} onChange={(e) => setConfirmPassword(e.target.value)} required />

            <button className="btn btn-primary" type="submit" disabled={busy || !username || password.length < 10 || !confirmPassword}>
              {busy ? 'Creating…' : 'Create Admin'}
            </button>
          </form>
          {message && <p className="auth-message">{message}</p>}
        </section>
      </main>
    </div>
  )
}

function LoginView({ onLoggedIn }: { onLoggedIn: () => Promise<void> | void }) {
  const [mode, setMode] = useState<'login' | 'register' | 'reset'>('login')
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const [inviteCode, setInviteCode] = useState('')
  const [newPassword, setNewPassword] = useState('')
  const [resetCode, setResetCode] = useState('')
  const [busy, setBusy] = useState(false)
  const [message, setMessage] = useState<string | null>(null)

  async function submit(e: React.FormEvent) {
    e.preventDefault()
    setBusy(true)
    setMessage(null)
    try {
      if (mode === 'login') {
        const res = await apiPost('/api/auth/login', { username, password })
        if (res.success) {
          await onLoggedIn()
        } else {
          setMessage(res.message ?? 'Login failed')
        }
      } else if (mode === 'register') {
        const res = await apiPost('/api/auth/register', { invite_code: inviteCode, username, password })
        if (res.success) {
          setMessage('Registration successful. You can sign in now.')
          setMode('login')
          setPassword('')
        } else {
          setMessage(res.message ?? 'Registration failed')
        }
      } else {
        const res = await apiPost('/api/auth/reset-password', { reset_code: resetCode, new_password: newPassword })
        if (res.success) {
          setMessage('Password updated. You can sign in now.')
          setMode('login')
          setPassword('')
          setNewPassword('')
          setResetCode('')
        } else {
          setMessage(res.message ?? 'Password reset failed')
        }
      }
    } catch (err) {
      setMessage(err instanceof Error ? err.message : 'Request failed')
    } finally {
      setBusy(false)
    }
  }

  return (
    <div className="app-shell">
      <main className="app-main auth-main">
        <section className="card auth-card">
          <h2 className="auth-title">Account Access</h2>
          <p className="text-faint auth-subtitle">
            {mode === 'login' && 'Use your dashboard account to continue.'}
            {mode === 'register' && 'Create your account with an invite code from an admin.'}
            {mode === 'reset' && 'Use a one-time reset code created by an admin.'}
          </p>
          <div className="auth-switch-row">
            <button type="button" className={`btn btn-sm ${mode === 'login' ? 'btn-primary' : ''}`} onClick={() => setMode('login')}>Sign In</button>
            <button type="button" className={`btn btn-sm ${mode === 'register' ? 'btn-primary' : ''}`} onClick={() => setMode('register')}>Register</button>
            <button type="button" className={`btn btn-sm ${mode === 'reset' ? 'btn-primary' : ''}`} onClick={() => setMode('reset')}>Reset</button>
          </div>
          <form className="auth-form" onSubmit={submit}>
            <label className="field-label">Username</label>
            <input className="input" value={username} onChange={(e) => setUsername(e.target.value)} required />

            {mode === 'register' && (
              <>
                <label className="field-label">Invite Code</label>
                <input className="input input-mono" value={inviteCode} onChange={(e) => setInviteCode(e.target.value)} required />
              </>
            )}

            {(mode === 'login' || mode === 'register') && (
              <>
                <label className="field-label">Password</label>
                <input className="input" type="password" value={password} onChange={(e) => setPassword(e.target.value)} required />
              </>
            )}

            {mode === 'reset' && (
              <>
                <label className="field-label">Reset Code</label>
                <input className="input input-mono" value={resetCode} onChange={(e) => setResetCode(e.target.value)} required />

                <label className="field-label">New Password</label>
                <input className="input" type="password" value={newPassword} onChange={(e) => setNewPassword(e.target.value)} required />
              </>
            )}

            <button
              className="btn btn-primary"
              type="submit"
              disabled={
                busy
                || !username
                || (mode === 'login' && !password)
                || (mode === 'register' && (!password || !inviteCode))
                || (mode === 'reset' && (!resetCode || !newPassword))
              }
            >
              {busy ? 'Working…' : mode === 'login' ? 'Sign In' : mode === 'register' ? 'Register Account' : 'Reset Password'}
            </button>
          </form>
          {message && <p className="auth-message">{message}</p>}
        </section>
      </main>
    </div>
  )
}
