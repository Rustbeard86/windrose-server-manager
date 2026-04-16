import type { SessionInfo } from '../types/api'

export const PERM_VIEW_DASHBOARD = 1 << 0
export const PERM_MANAGE_SERVER = 1 << 1
export const PERM_MANAGE_CONFIG = 1 << 2
export const PERM_MANAGE_BACKUPS = 1 << 3
export const PERM_MANAGE_INSTALL = 1 << 4
export const PERM_MANAGE_UPDATES = 1 << 5
export const PERM_MANAGE_SCHEDULE = 1 << 6
export const PERM_MANAGE_USERS = 1 << 7

export function hasPermission(session: SessionInfo | null, permission: number): boolean {
  if (!session) return false
  return session.is_admin || (session.permission_flags & permission) === permission
}
