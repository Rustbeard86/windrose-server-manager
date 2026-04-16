import { useEffect, useState } from 'react'
import type { AuditEventSummary, AuthUserSummary, CreatedInvite, InviteSummary } from '../types/api'
import { apiGet, apiPost, apiPut } from '../utils/api'
import {
  PERM_MANAGE_BACKUPS,
  PERM_MANAGE_CONFIG,
  PERM_MANAGE_INSTALL,
  PERM_MANAGE_SCHEDULE,
  PERM_MANAGE_SERVER,
  PERM_MANAGE_UPDATES,
  PERM_MANAGE_USERS,
  PERM_VIEW_DASHBOARD,
} from '../utils/permissions'
import './AuthAdminView.css'

interface AuthAdminViewProps {
  canManageUsers: boolean
}

interface UserEditState {
  is_admin: boolean
  permission_flags: string
  disabled: boolean
}

const PERMISSION_OPTIONS: Array<{ label: string; mask: number }> = [
  { label: 'View Dashboard', mask: PERM_VIEW_DASHBOARD },
  { label: 'Manage Server', mask: PERM_MANAGE_SERVER },
  { label: 'Manage Config', mask: PERM_MANAGE_CONFIG },
  { label: 'Manage Backups', mask: PERM_MANAGE_BACKUPS },
  { label: 'Manage Install', mask: PERM_MANAGE_INSTALL },
  { label: 'Manage Updates', mask: PERM_MANAGE_UPDATES },
  { label: 'Manage Schedule', mask: PERM_MANAGE_SCHEDULE },
  { label: 'Manage Users', mask: PERM_MANAGE_USERS },
]

function parseFlags(value: string): bigint {
  const trimmed = value.trim()
  if (!trimmed) return 0n
  try {
    return BigInt(trimmed)
  } catch {
    return 0n
  }
}

function hasFlag(flags: string, mask: number): boolean {
  const maskBig = BigInt(mask)
  return (parseFlags(flags) & maskBig) === maskBig
}

function updateFlag(flags: string, mask: number, enabled: boolean): string {
  const maskBig = BigInt(mask)
  const current = parseFlags(flags)
  const next = enabled ? (current | maskBig) : (current & ~maskBig)
  return next.toString()
}

export function AuthAdminView({ canManageUsers }: AuthAdminViewProps) {
  const [users, setUsers] = useState<AuthUserSummary[]>([])
  const [invites, setInvites] = useState<InviteSummary[]>([])
  const [audit, setAudit] = useState<AuditEventSummary[]>([])
  const [busy, setBusy] = useState(false)
  const [savingUserId, setSavingUserId] = useState<number | null>(null)
  const [message, setMessage] = useState<string | null>(null)
  const [userEdits, setUserEdits] = useState<Record<number, UserEditState>>({})

  const [inviteFlags, setInviteFlags] = useState<string>('1')
  const [inviteMaxUses, setInviteMaxUses] = useState<string>('1')
  const [inviteExpiresHours, setInviteExpiresHours] = useState<string>('24')
  const [lastInvite, setLastInvite] = useState<CreatedInvite | null>(null)

  const [resetUsername, setResetUsername] = useState('')
  const [resetTtlMins, setResetTtlMins] = useState('30')
  const [lastResetCode, setLastResetCode] = useState<string | null>(null)

  async function loadAll() {
    if (!canManageUsers) return
    setBusy(true)
    setMessage(null)
    try {
      const [usersRes, invitesRes, auditRes] = await Promise.all([
        apiGet<AuthUserSummary[]>('/api/auth/users'),
        apiGet<InviteSummary[]>('/api/auth/invites'),
        apiGet<AuditEventSummary[]>('/api/auth/audit?limit=150'),
      ])
      if (usersRes.success && usersRes.data) {
        setUsers(usersRes.data)
        const mapped = usersRes.data.reduce<Record<number, UserEditState>>((acc, user) => {
          acc[user.id] = {
            is_admin: user.is_admin,
            permission_flags: String(user.permission_flags),
            disabled: user.disabled,
          }
          return acc
        }, {})
        setUserEdits(mapped)
      }
      if (invitesRes.success && invitesRes.data) setInvites(invitesRes.data)
      if (auditRes.success && auditRes.data) setAudit(auditRes.data)
    } catch (e) {
      setMessage(e instanceof Error ? e.message : 'Failed to load auth admin data')
    } finally {
      setBusy(false)
    }
  }

  async function saveUser(userId: number) {
    const edit = userEdits[userId]
    if (!edit) return
    setMessage(null)
    setSavingUserId(userId)
    try {
      const res = await apiPut<AuthUserSummary>(`/api/auth/users/${userId}`, {
        is_admin: edit.is_admin,
        permission_flags: Number(edit.permission_flags) || 0,
        disabled: edit.disabled,
      })
      if (!res.success) {
        setMessage(res.message ?? 'Failed to update user')
        return
      }
      setMessage('User updated')
      await loadAll()
    } catch (e) {
      setMessage(e instanceof Error ? e.message : 'Failed to update user')
    } finally {
      setSavingUserId(null)
    }
  }

  useEffect(() => {
    void loadAll()
  }, [canManageUsers])

  async function createInvite() {
    setMessage(null)
    setLastInvite(null)
    try {
      const res = await apiPost<CreatedInvite>('/api/auth/invites', {
        permission_flags: Number(inviteFlags) || 0,
        max_uses: Number(inviteMaxUses) || 1,
        expires_in_hours: Number(inviteExpiresHours) || 24,
      })
      if (!res.success || !res.data) {
        setMessage(res.message ?? 'Failed to create invite')
        return
      }
      setLastInvite(res.data)
      await loadAll()
    } catch (e) {
      setMessage(e instanceof Error ? e.message : 'Failed to create invite')
    }
  }

  async function createResetCode() {
    if (!resetUsername.trim()) {
      setMessage('Username is required')
      return
    }
    setMessage(null)
    setLastResetCode(null)
    try {
      const res = await apiPost<{ code: string; expires_at: number }>('/api/auth/reset-code', {
        username: resetUsername.trim(),
        expires_in_minutes: Number(resetTtlMins) || 30,
      })
      if (!res.success || !res.data) {
        setMessage(res.message ?? 'Failed to create reset code')
        return
      }
      setLastResetCode(`${res.data.code} (expires at unix ${res.data.expires_at})`)
      await loadAll()
    } catch (e) {
      setMessage(e instanceof Error ? e.message : 'Failed to create reset code')
    }
  }

  if (!canManageUsers) {
    return (
      <div className="card auth-admin-card">
        <div className="panel-title">User & Auth Administration</div>
        <p className="text-faint">Your account does not have permission to manage users.</p>
      </div>
    )
  }

  return (
    <div className="auth-admin-view animate-fade-in">
      <section className="card auth-admin-card">
        <div className="panel-title">Create Invite</div>
        <div className="auth-admin-form-grid">
          <label className="field-label">Permission Flags</label>
          <input className="input input-mono" value={inviteFlags} onChange={(e) => setInviteFlags(e.target.value)} />

          <label className="field-label">Max Uses</label>
          <input className="input input-mono" value={inviteMaxUses} onChange={(e) => setInviteMaxUses(e.target.value)} />

          <label className="field-label">Expires In Hours</label>
          <input className="input input-mono" value={inviteExpiresHours} onChange={(e) => setInviteExpiresHours(e.target.value)} />
        </div>
        <div className="auth-admin-actions">
          <button className="btn btn-primary" onClick={() => void createInvite()} disabled={busy}>Create Invite</button>
          <button className="btn" onClick={() => void loadAll()} disabled={busy}>Refresh</button>
        </div>
        {lastInvite && (
          <p className="auth-admin-result text-success">
            Invite code: <strong>{lastInvite.code}</strong>
          </p>
        )}
      </section>

      <section className="card auth-admin-card">
        <div className="panel-title">Create Reset Code</div>
        <div className="auth-admin-form-grid">
          <label className="field-label">Username</label>
          <input className="input" value={resetUsername} onChange={(e) => setResetUsername(e.target.value)} />

          <label className="field-label">Expires In Minutes</label>
          <input className="input input-mono" value={resetTtlMins} onChange={(e) => setResetTtlMins(e.target.value)} />
        </div>
        <div className="auth-admin-actions">
          <button className="btn btn-primary" onClick={() => void createResetCode()} disabled={busy}>Create Reset Code</button>
        </div>
        {lastResetCode && <p className="auth-admin-result text-warning">{lastResetCode}</p>}
      </section>

      <section className="card auth-admin-card">
        <div className="panel-title">Users ({users.length})</div>
        <div className="auth-admin-list">
          {users.map((u) => (
            <div key={u.id} className="auth-admin-user-row">
              <div className="auth-admin-user-head">
                <span className="auth-admin-username">{u.username}</span>
                <span className={`badge ${u.disabled ? 'badge-crashed' : 'badge-running'}`}>{u.disabled ? 'disabled' : 'active'}</span>
              </div>
              <div className="auth-admin-user-grid">
                <label className="auth-admin-checkline">
                  <input
                    type="checkbox"
                    checked={userEdits[u.id]?.is_admin ?? u.is_admin}
                    onChange={(e) => setUserEdits((prev) => ({
                      ...prev,
                      [u.id]: {
                        ...(prev[u.id] ?? {
                          is_admin: u.is_admin,
                          permission_flags: String(u.permission_flags),
                          disabled: u.disabled,
                        }),
                        is_admin: e.target.checked,
                      },
                    }))}
                  />
                  <span>Admin</span>
                </label>

                <label className="auth-admin-checkline">
                  <input
                    type="checkbox"
                    checked={userEdits[u.id]?.disabled ?? u.disabled}
                    onChange={(e) => setUserEdits((prev) => ({
                      ...prev,
                      [u.id]: {
                        ...(prev[u.id] ?? {
                          is_admin: u.is_admin,
                          permission_flags: String(u.permission_flags),
                          disabled: u.disabled,
                        }),
                        disabled: e.target.checked,
                      },
                    }))}
                  />
                  <span>Disabled</span>
                </label>

                <label className="field-label">Permission Flags</label>
                <input
                  className="input input-mono"
                  value={userEdits[u.id]?.permission_flags ?? String(u.permission_flags)}
                  onChange={(e) => setUserEdits((prev) => ({
                    ...prev,
                    [u.id]: {
                      ...(prev[u.id] ?? {
                        is_admin: u.is_admin,
                        permission_flags: String(u.permission_flags),
                        disabled: u.disabled,
                      }),
                      permission_flags: e.target.value,
                    },
                  }))}
                />

                <div className="auth-admin-permission-grid">
                  {PERMISSION_OPTIONS.map((perm) => {
                    const currentFlags = userEdits[u.id]?.permission_flags ?? String(u.permission_flags)
                    return (
                      <label key={`${u.id}-${perm.mask}`} className="auth-admin-checkline auth-admin-perm-item">
                        <input
                          type="checkbox"
                          checked={hasFlag(currentFlags, perm.mask)}
                          onChange={(e) => setUserEdits((prev) => ({
                            ...prev,
                            [u.id]: {
                              ...(prev[u.id] ?? {
                                is_admin: u.is_admin,
                                permission_flags: String(u.permission_flags),
                                disabled: u.disabled,
                              }),
                              permission_flags: updateFlag(currentFlags, perm.mask, e.target.checked),
                            },
                          }))}
                        />
                        <span>{perm.label}</span>
                      </label>
                    )
                  })}
                </div>

                <button
                  className="btn btn-sm btn-primary"
                  onClick={() => void saveUser(u.id)}
                  disabled={busy || savingUserId === u.id}
                >
                  {savingUserId === u.id ? 'Saving…' : 'Save User'}
                </button>
              </div>
            </div>
          ))}
        </div>
      </section>

      <section className="card auth-admin-card">
        <div className="panel-title">Invites ({invites.length})</div>
        <div className="auth-admin-list">
          {invites.map((inv) => (
            <div key={inv.id} className="auth-admin-row">
              <span>#{inv.id}</span>
              <span className="text-faint">flags={inv.permission_flags}</span>
              <span className="text-faint">{inv.uses}/{inv.max_uses}</span>
              <span className={`badge ${inv.expired || inv.exhausted ? 'badge-stopped' : 'badge-running'}`}>
                {inv.expired ? 'expired' : inv.exhausted ? 'used' : 'active'}
              </span>
            </div>
          ))}
        </div>
      </section>

      <section className="card auth-admin-card auth-admin-audit">
        <div className="panel-title">Audit ({audit.length})</div>
        <div className="auth-admin-list">
          {audit.map((e) => (
            <div key={e.id} className="auth-admin-row auth-admin-row-audit">
              <span className="auth-admin-audit-time">{new Date(e.created_at * 1000).toLocaleString()}</span>
              <span>{e.action}</span>
              <span className="text-faint">{e.actor_username ?? 'system'}</span>
              <span className={`badge ${e.success ? 'badge-running' : 'badge-crashed'}`}>{e.success ? 'ok' : 'fail'}</span>
            </div>
          ))}
        </div>
      </section>

      {message && <p className="auth-admin-message text-danger">{message}</p>}
    </div>
  )
}
