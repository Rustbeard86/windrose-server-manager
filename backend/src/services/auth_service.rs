use std::path::{Path, PathBuf};

use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rand::{distributions::Alphanumeric, rngs::OsRng, Rng};
use rusqlite::{params, OptionalExtension};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone)]
pub struct AuthSettings {
    pub session_ttl_secs: i64,
    pub max_failed_logins: i64,
    pub lockout_minutes: i64,
}

impl AuthSettings {
    pub fn sanitised(self) -> Self {
        Self {
            session_ttl_secs: self.session_ttl_secs.clamp(15 * 60, 7 * 24 * 60 * 60),
            max_failed_logins: self.max_failed_logins.clamp(1, 50),
            lockout_minutes: self.lockout_minutes.clamp(1, 24 * 60),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AuthUserRecord {
    pub id: i64,
    pub username: String,
    pub is_admin: bool,
    pub permission_flags: i64,
    pub disabled: bool,
    pub created_at: i64,
}

#[derive(Debug, Clone)]
pub struct InviteRecord {
    pub id: i64,
    pub permission_flags: i64,
    pub max_uses: i64,
    pub uses: i64,
    pub created_at: i64,
    pub expires_at: Option<i64>,
    pub created_by_user: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct AuditEventRecord {
    pub id: i64,
    pub created_at: i64,
    pub actor_user_id: Option<i64>,
    pub actor_username: Option<String>,
    pub action: String,
    pub details: Option<String>,
    pub success: bool,
}

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: i64,
    pub username: String,
    pub is_admin: bool,
    pub permission_flags: i64,
}

#[derive(Clone)]
pub struct AuthService {
    pool: Pool<SqliteConnectionManager>,
    settings: AuthSettings,
}

impl std::fmt::Debug for AuthService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthService").finish_non_exhaustive()
    }
}

impl AuthService {
    pub fn new(base_dir: Option<PathBuf>, settings: AuthSettings) -> Result<Self, String> {
        let db_path = auth_db_path(base_dir)
            .ok_or_else(|| "Cannot resolve auth database path".to_string())?;
        let manager = SqliteConnectionManager::file(db_path);
        let pool = Pool::builder()
            .max_size(8)
            .build(manager)
            .map_err(|e| format!("Failed to open auth DB: {e}"))?;

        let svc = Self {
            pool,
            settings: settings.sanitised(),
        };
        svc.init_schema()?;
        Ok(svc)
    }

    fn init_schema(&self) -> Result<(), String> {
        let conn = self
            .pool
            .get()
            .map_err(|e| format!("Auth DB checkout failed: {e}"))?;

        conn.execute_batch(
            "
            PRAGMA journal_mode=WAL;
            PRAGMA foreign_keys=ON;

            CREATE TABLE IF NOT EXISTS users (
                id               INTEGER PRIMARY KEY AUTOINCREMENT,
                username         TEXT NOT NULL UNIQUE,
                password_hash    TEXT NOT NULL,
                is_admin         INTEGER NOT NULL DEFAULT 0,
                permission_flags INTEGER NOT NULL DEFAULT 0,
                disabled         INTEGER NOT NULL DEFAULT 0,
                created_at       INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS sessions (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id     INTEGER NOT NULL,
                token_hash  TEXT NOT NULL UNIQUE,
                created_at  INTEGER NOT NULL,
                expires_at  INTEGER NOT NULL,
                FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS invites (
                id               INTEGER PRIMARY KEY AUTOINCREMENT,
                code_hash        TEXT NOT NULL UNIQUE,
                permission_flags INTEGER NOT NULL,
                max_uses         INTEGER NOT NULL DEFAULT 1,
                uses             INTEGER NOT NULL DEFAULT 0,
                created_at       INTEGER NOT NULL,
                expires_at       INTEGER,
                created_by_user  INTEGER
            );

            CREATE TABLE IF NOT EXISTS password_reset_codes (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id     INTEGER NOT NULL,
                code_hash   TEXT NOT NULL UNIQUE,
                created_at  INTEGER NOT NULL,
                expires_at  INTEGER NOT NULL,
                consumed_at INTEGER,
                created_by_user INTEGER,
                FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS audit_events (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                created_at  INTEGER NOT NULL,
                actor_user_id INTEGER,
                actor_username TEXT,
                action      TEXT NOT NULL,
                details     TEXT,
                success     INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS login_failures (
                username       TEXT PRIMARY KEY,
                failed_count   INTEGER NOT NULL DEFAULT 0,
                lockout_until  INTEGER
            );
            ",
        )
        .map_err(|e| format!("Auth schema init failed: {e}"))?;

        Ok(())
    }

    pub fn has_users(&self) -> Result<bool, String> {
        let conn = self
            .pool
            .get()
            .map_err(|e| format!("Auth DB checkout failed: {e}"))?;
        let count: i64 = conn
            .query_row("SELECT COUNT(1) FROM users", [], |r| r.get(0))
            .map_err(|e| format!("Auth users count failed: {e}"))?;
        Ok(count > 0)
    }

    pub fn bootstrap_admin(&self, username: &str, password: &str) -> Result<(), String> {
        let username = username.trim();
        if username.is_empty() {
            return Err("Username is required".to_string());
        }
        if password.len() < 10 {
            return Err("Password must be at least 10 characters".to_string());
        }

        let conn = self
            .pool
            .get()
            .map_err(|e| format!("Auth DB checkout failed: {e}"))?;

        let count: i64 = conn
            .query_row("SELECT COUNT(1) FROM users", [], |r| r.get(0))
            .map_err(|e| format!("Auth users count failed: {e}"))?;
        if count > 0 {
            return Err("Bootstrap is disabled after first user is created".to_string());
        }

        let hash = hash_password(password)?;
        let now = Utc::now().timestamp();
        conn.execute(
            "INSERT INTO users (username, password_hash, is_admin, permission_flags, created_at)
             VALUES (?1, ?2, 1, ?3, ?4)",
            params![username, hash, i64::MAX, now],
        )
        .map_err(|e| format!("Failed to create bootstrap admin: {e}"))?;

        self.audit(None, Some(username), "auth.bootstrap_admin", "bootstrap admin created", true)?;
        Ok(())
    }

    pub fn login(&self, username: &str, password: &str) -> Result<(String, AuthUser), String> {
        let conn = self
            .pool
            .get()
            .map_err(|e| format!("Auth DB checkout failed: {e}"))?;

        let now = Utc::now().timestamp();
        let lockout_until: Option<i64> = conn
            .query_row(
                "SELECT lockout_until FROM login_failures WHERE username = ?1",
                [username],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| format!("Login lockout lookup failed: {e}"))?
            .flatten();
        if let Some(until) = lockout_until {
            if until > now {
                self.audit(None, Some(username), "auth.login", "account lockout active", false)?;
                return Err("Too many failed attempts. Try again later.".to_string());
            }
        }

        let row: Option<(i64, String, bool, i64, bool)> = conn
            .query_row(
                "SELECT id, password_hash, is_admin, permission_flags, disabled
                 FROM users WHERE username = ?1",
                [username],
                |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, i64>(2)? != 0,
                        r.get::<_, i64>(3)?,
                        r.get::<_, i64>(4)? != 0,
                    ))
                },
            )
            .optional()
            .map_err(|e| format!("User lookup failed: {e}"))?;

        let Some((user_id, password_hash, is_admin, permission_flags, disabled)) = row else {
            self.record_login_failure(&conn, username, now)?;
            self.audit(None, Some(username), "auth.login", "unknown user", false)?;
            return Err("Invalid username or password".to_string());
        };

        if disabled {
            self.audit(Some(user_id), Some(username), "auth.login", "account disabled", false)?;
            return Err("Account is disabled".to_string());
        }

        if !verify_password(password, &password_hash)? {
            self.record_login_failure(&conn, username, now)?;
            self.audit(Some(user_id), Some(username), "auth.login", "bad password", false)?;
            return Err("Invalid username or password".to_string());
        }

        conn.execute("DELETE FROM login_failures WHERE username = ?1", [username])
            .map_err(|e| format!("Failed to clear login failures: {e}"))?;

        let token = random_token(64);
        let token_hash = hash_token(&token);
        let expires = now + self.settings.session_ttl_secs;

        conn.execute(
            "INSERT INTO sessions (user_id, token_hash, created_at, expires_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![user_id, token_hash, now, expires],
        )
        .map_err(|e| format!("Failed to create session: {e}"))?;

        self.audit(Some(user_id), Some(username), "auth.login", "success", true)?;

        Ok((
            token,
            AuthUser {
                id: user_id,
                username: username.to_string(),
                is_admin,
                permission_flags,
            },
        ))
    }

    fn record_login_failure(
        &self,
        conn: &rusqlite::Connection,
        username: &str,
        now: i64,
    ) -> Result<(), String> {
        let row: Option<(i64, Option<i64>)> = conn
            .query_row(
                "SELECT failed_count, lockout_until FROM login_failures WHERE username = ?1",
                [username],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(|e| format!("Failed to read login failure state: {e}"))?;

        let mut failed = row.as_ref().map(|(f, _)| *f).unwrap_or(0) + 1;
        let mut lockout_until = row.and_then(|(_, until)| until);
        if failed >= self.settings.max_failed_logins {
            lockout_until = Some(now + (self.settings.lockout_minutes * 60));
            failed = 0;
        }

        conn.execute(
            "INSERT INTO login_failures (username, failed_count, lockout_until)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(username) DO UPDATE SET
               failed_count = excluded.failed_count,
               lockout_until = excluded.lockout_until",
            params![username, failed, lockout_until],
        )
        .map_err(|e| format!("Failed to write login failure state: {e}"))?;

        Ok(())
    }

    pub fn list_users(&self) -> Result<Vec<AuthUserRecord>, String> {
        let conn = self
            .pool
            .get()
            .map_err(|e| format!("Auth DB checkout failed: {e}"))?;

        let mut stmt = conn
            .prepare(
                "SELECT id, username, is_admin, permission_flags, disabled, created_at
                 FROM users
                 ORDER BY created_at ASC, id ASC",
            )
            .map_err(|e| format!("Failed to prepare users query: {e}"))?;

        let rows = stmt
            .query_map([], |r| {
                Ok(AuthUserRecord {
                    id: r.get(0)?,
                    username: r.get(1)?,
                    is_admin: r.get::<_, i64>(2)? != 0,
                    permission_flags: r.get(3)?,
                    disabled: r.get::<_, i64>(4)? != 0,
                    created_at: r.get(5)?,
                })
            })
            .map_err(|e| format!("Failed to query users: {e}"))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("Failed to read users: {e}"))
    }

    pub fn update_user(
        &self,
        actor_user_id: i64,
        target_user_id: i64,
        is_admin: Option<bool>,
        permission_flags: Option<i64>,
        disabled: Option<bool>,
    ) -> Result<AuthUserRecord, String> {
        let conn = self
            .pool
            .get()
            .map_err(|e| format!("Auth DB checkout failed: {e}"))?;

        let row: Option<(String, bool, i64, bool, i64)> = conn
            .query_row(
                "SELECT username, is_admin, permission_flags, disabled, created_at
                 FROM users
                 WHERE id = ?1",
                [target_user_id],
                |r| {
                    Ok((
                        r.get(0)?,
                        r.get::<_, i64>(1)? != 0,
                        r.get(2)?,
                        r.get::<_, i64>(3)? != 0,
                        r.get(4)?,
                    ))
                },
            )
            .optional()
            .map_err(|e| format!("Failed to look up user: {e}"))?;

        let Some((username, current_is_admin, current_flags, current_disabled, created_at)) = row else {
            return Err("User not found".to_string());
        };

        let next_is_admin = is_admin.unwrap_or(current_is_admin);
        let next_flags = permission_flags.unwrap_or(current_flags);
        let next_disabled = disabled.unwrap_or(current_disabled);

        if actor_user_id == target_user_id && (next_disabled || !next_is_admin) {
            return Err("Cannot remove your own admin access or disable your own account".to_string());
        }

        if current_is_admin && (!next_is_admin || (!current_disabled && next_disabled)) {
            let other_admin_count: i64 = conn
                .query_row(
                    "SELECT COUNT(1)
                     FROM users
                     WHERE is_admin = 1
                       AND disabled = 0
                       AND id != ?1",
                    [target_user_id],
                    |r| r.get(0),
                )
                .map_err(|e| format!("Failed to validate admin safety: {e}"))?;
            if other_admin_count == 0 {
                return Err("Cannot remove or disable the last active admin".to_string());
            }
        }

        conn.execute(
            "UPDATE users
             SET is_admin = ?1,
                 permission_flags = ?2,
                 disabled = ?3
             WHERE id = ?4",
            params![
                if next_is_admin { 1 } else { 0 },
                next_flags,
                if next_disabled { 1 } else { 0 },
                target_user_id
            ],
        )
        .map_err(|e| format!("Failed to update user: {e}"))?;

        if next_disabled {
            conn.execute("DELETE FROM sessions WHERE user_id = ?1", [target_user_id])
                .map_err(|e| format!("Failed to invalidate disabled user sessions: {e}"))?;
        }

        self.audit(
            Some(actor_user_id),
            None,
            "auth.user.update",
            &format!(
                "target_user_id={target_user_id}, is_admin={next_is_admin}, permission_flags={next_flags}, disabled={next_disabled}"
            ),
            true,
        )?;

        Ok(AuthUserRecord {
            id: target_user_id,
            username,
            is_admin: next_is_admin,
            permission_flags: next_flags,
            disabled: next_disabled,
            created_at,
        })
    }

    pub fn create_invite(
        &self,
        actor_user_id: Option<i64>,
        permission_flags: i64,
        max_uses: i64,
        expires_at: Option<i64>,
    ) -> Result<String, String> {
        if max_uses < 1 {
            return Err("max_uses must be at least 1".to_string());
        }

        let conn = self
            .pool
            .get()
            .map_err(|e| format!("Auth DB checkout failed: {e}"))?;

        let code = random_token(32);
        let code_hash = hash_token(&code);
        let now = Utc::now().timestamp();

        conn.execute(
            "INSERT INTO invites (code_hash, permission_flags, max_uses, uses, created_at, expires_at, created_by_user)
             VALUES (?1, ?2, ?3, 0, ?4, ?5, ?6)",
            params![
                code_hash,
                permission_flags,
                max_uses,
                now,
                expires_at,
                actor_user_id
            ],
        )
        .map_err(|e| format!("Failed to create invite: {e}"))?;

        self.audit(
            actor_user_id,
            None,
            "auth.invite.create",
            &format!(
                "permission_flags={permission_flags}, max_uses={max_uses}, expires_at={:?}",
                expires_at
            ),
            true,
        )?;

        Ok(code)
    }

    pub fn list_invites(&self) -> Result<Vec<InviteRecord>, String> {
        let conn = self
            .pool
            .get()
            .map_err(|e| format!("Auth DB checkout failed: {e}"))?;

        let mut stmt = conn
            .prepare(
                "SELECT id, permission_flags, max_uses, uses, created_at, expires_at, created_by_user
                 FROM invites
                 ORDER BY created_at DESC, id DESC",
            )
            .map_err(|e| format!("Failed to prepare invites query: {e}"))?;

        let rows = stmt
            .query_map([], |r| {
                Ok(InviteRecord {
                    id: r.get(0)?,
                    permission_flags: r.get(1)?,
                    max_uses: r.get(2)?,
                    uses: r.get(3)?,
                    created_at: r.get(4)?,
                    expires_at: r.get(5)?,
                    created_by_user: r.get(6)?,
                })
            })
            .map_err(|e| format!("Failed to query invites: {e}"))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("Failed to read invites: {e}"))
    }

    pub fn register_with_invite(
        &self,
        invite_code: &str,
        username: &str,
        password: &str,
    ) -> Result<(), String> {
        let username = username.trim();
        if username.is_empty() {
            return Err("Username is required".to_string());
        }
        if password.len() < 10 {
            return Err("Password must be at least 10 characters".to_string());
        }

        let conn = self
            .pool
            .get()
            .map_err(|e| format!("Auth DB checkout failed: {e}"))?;

        let now = Utc::now().timestamp();
        let code_hash = hash_token(invite_code);

        let invite = conn
            .query_row(
                "SELECT id, permission_flags, max_uses, uses, expires_at
                 FROM invites
                 WHERE code_hash = ?1",
                [code_hash],
                |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, i64>(1)?,
                        r.get::<_, i64>(2)?,
                        r.get::<_, i64>(3)?,
                        r.get::<_, Option<i64>>(4)?,
                    ))
                },
            )
            .optional()
            .map_err(|e| format!("Failed to look up invite: {e}"))?;

        let Some((invite_id, permission_flags, max_uses, uses, expires_at)) = invite else {
            return Err("Invalid invite code".to_string());
        };

        if uses >= max_uses {
            return Err("Invite code has already been used up".to_string());
        }
        if let Some(expiry) = expires_at {
            if now >= expiry {
                return Err("Invite code has expired".to_string());
            }
        }

        let tx = conn
            .unchecked_transaction()
            .map_err(|e| format!("Failed to start transaction: {e}"))?;

        let existing: Option<i64> = tx
            .query_row("SELECT id FROM users WHERE username = ?1", [username], |r| r.get(0))
            .optional()
            .map_err(|e| format!("Failed to validate username: {e}"))?;
        if existing.is_some() {
            return Err("Username is already taken".to_string());
        }

        let password_hash = hash_password(password)?;
        tx.execute(
            "INSERT INTO users (username, password_hash, is_admin, permission_flags, disabled, created_at)
             VALUES (?1, ?2, 0, ?3, 0, ?4)",
            params![username, password_hash, permission_flags, now],
        )
        .map_err(|e| format!("Failed to create user from invite: {e}"))?;

        tx.execute(
            "UPDATE invites SET uses = uses + 1 WHERE id = ?1",
            [invite_id],
        )
        .map_err(|e| format!("Failed to consume invite: {e}"))?;

        tx.commit()
            .map_err(|e| format!("Failed to commit invite registration: {e}"))?;

        self.audit(
            None,
            Some(username),
            "auth.register_with_invite",
            &format!("invite_id={invite_id}"),
            true,
        )?;

        Ok(())
    }

    pub fn create_reset_code(
        &self,
        actor_user_id: Option<i64>,
        username: &str,
        expires_at: i64,
    ) -> Result<String, String> {
        let conn = self
            .pool
            .get()
            .map_err(|e| format!("Auth DB checkout failed: {e}"))?;

        let user: Option<(i64, String)> = conn
            .query_row(
                "SELECT id, username FROM users WHERE username = ?1",
                [username],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(|e| format!("Failed to look up user for reset code: {e}"))?;

        let Some((user_id, canonical_username)) = user else {
            return Err("User not found".to_string());
        };

        let code = random_token(24);
        let code_hash = hash_token(&code);
        let now = Utc::now().timestamp();

        conn.execute(
            "INSERT INTO password_reset_codes (user_id, code_hash, created_at, expires_at, consumed_at, created_by_user)
             VALUES (?1, ?2, ?3, ?4, NULL, ?5)",
            params![user_id, code_hash, now, expires_at, actor_user_id],
        )
        .map_err(|e| format!("Failed to create password reset code: {e}"))?;

        self.audit(
            actor_user_id,
            None,
            "auth.reset_code.create",
            &format!("target_user_id={user_id}, username={canonical_username}"),
            true,
        )?;

        Ok(code)
    }

    pub fn reset_password(&self, code: &str, new_password: &str) -> Result<(), String> {
        if new_password.len() < 10 {
            return Err("Password must be at least 10 characters".to_string());
        }

        let conn = self
            .pool
            .get()
            .map_err(|e| format!("Auth DB checkout failed: {e}"))?;

        let now = Utc::now().timestamp();
        let code_hash = hash_token(code);

        let reset_row: Option<(i64, i64)> = conn
            .query_row(
                "SELECT id, user_id
                 FROM password_reset_codes
                 WHERE code_hash = ?1
                   AND consumed_at IS NULL
                   AND expires_at > ?2",
                params![code_hash, now],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(|e| format!("Failed to look up password reset code: {e}"))?;

        let Some((reset_id, user_id)) = reset_row else {
            return Err("Invalid or expired reset code".to_string());
        };

        let tx = conn
            .unchecked_transaction()
            .map_err(|e| format!("Failed to start transaction: {e}"))?;

        let new_hash = hash_password(new_password)?;

        tx.execute(
            "UPDATE users SET password_hash = ?1 WHERE id = ?2",
            params![new_hash, user_id],
        )
        .map_err(|e| format!("Failed to update password: {e}"))?;

        tx.execute(
            "UPDATE password_reset_codes SET consumed_at = ?1 WHERE id = ?2",
            params![now, reset_id],
        )
        .map_err(|e| format!("Failed to consume reset code: {e}"))?;

        tx.execute("DELETE FROM sessions WHERE user_id = ?1", [user_id])
            .map_err(|e| format!("Failed to invalidate sessions: {e}"))?;

        tx.commit()
            .map_err(|e| format!("Failed to commit password reset: {e}"))?;

        self.audit(
            Some(user_id),
            None,
            "auth.password.reset",
            "password reset via code",
            true,
        )?;

        Ok(())
    }

    pub fn list_audit_events(&self, limit: usize) -> Result<Vec<AuditEventRecord>, String> {
        let conn = self
            .pool
            .get()
            .map_err(|e| format!("Auth DB checkout failed: {e}"))?;

        let limit = limit.clamp(1, 500);
        let mut stmt = conn
            .prepare(
                "SELECT id, created_at, actor_user_id, actor_username, action, details, success
                 FROM audit_events
                 ORDER BY created_at DESC, id DESC
                 LIMIT ?1",
            )
            .map_err(|e| format!("Failed to prepare audit query: {e}"))?;

        let rows = stmt
            .query_map([limit as i64], |r| {
                Ok(AuditEventRecord {
                    id: r.get(0)?,
                    created_at: r.get(1)?,
                    actor_user_id: r.get(2)?,
                    actor_username: r.get(3)?,
                    action: r.get(4)?,
                    details: r.get(5)?,
                    success: r.get::<_, i64>(6)? != 0,
                })
            })
            .map_err(|e| format!("Failed to query audit events: {e}"))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("Failed to read audit events: {e}"))
    }

    pub fn cleanup_audit_events(&self, retention_days: i64) -> Result<usize, String> {
        let retention_days = retention_days.clamp(1, 3650);
        let cutoff = Utc::now().timestamp() - (retention_days * 24 * 60 * 60);

        let conn = self
            .pool
            .get()
            .map_err(|e| format!("Auth DB checkout failed: {e}"))?;

        let deleted = conn
            .execute(
                "DELETE FROM audit_events WHERE created_at < ?1",
                params![cutoff],
            )
            .map_err(|e| format!("Failed to clean up audit events: {e}"))?;

        Ok(deleted)
    }

    pub fn validate_session(&self, token: &str) -> Result<Option<AuthUser>, String> {
        let token_hash = hash_token(token);
        let now = Utc::now().timestamp();
        let conn = self
            .pool
            .get()
            .map_err(|e| format!("Auth DB checkout failed: {e}"))?;

        // Eagerly clean expired sessions on read.
        conn.execute("DELETE FROM sessions WHERE expires_at <= ?1", [now])
            .map_err(|e| format!("Failed to prune sessions: {e}"))?;

        let user = conn
            .query_row(
                "SELECT u.id, u.username, u.is_admin, u.permission_flags
                 FROM sessions s
                 JOIN users u ON u.id = s.user_id
                 WHERE s.token_hash = ?1 AND s.expires_at > ?2 AND u.disabled = 0",
                params![token_hash, now],
                |r| {
                    Ok(AuthUser {
                        id: r.get(0)?,
                        username: r.get(1)?,
                        is_admin: r.get::<_, i64>(2)? != 0,
                        permission_flags: r.get(3)?,
                    })
                },
            )
            .optional()
            .map_err(|e| format!("Session validation failed: {e}"))?;

        Ok(user)
    }

    pub fn logout(&self, token: &str) -> Result<(), String> {
        let token_hash = hash_token(token);
        let conn = self
            .pool
            .get()
            .map_err(|e| format!("Auth DB checkout failed: {e}"))?;

        let _ = conn
            .execute("DELETE FROM sessions WHERE token_hash = ?1", [token_hash])
            .map_err(|e| format!("Failed to remove session: {e}"))?;

        Ok(())
    }

    pub fn audit(
        &self,
        actor_user_id: Option<i64>,
        actor_username: Option<&str>,
        action: &str,
        details: &str,
        success: bool,
    ) -> Result<(), String> {
        let conn = self
            .pool
            .get()
            .map_err(|e| format!("Auth DB checkout failed: {e}"))?;
        let now = Utc::now().timestamp();
        conn.execute(
            "INSERT INTO audit_events (created_at, actor_user_id, actor_username, action, details, success)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                now,
                actor_user_id,
                actor_username,
                action,
                details,
                if success { 1 } else { 0 }
            ],
        )
        .map_err(|e| format!("Failed to write audit event: {e}"))?;
        Ok(())
    }
}

fn auth_db_path(base_dir: Option<PathBuf>) -> Option<PathBuf> {
    let base = base_dir.or_else(crate::config::AppConfig::binary_dir)?;
    Some(base.join("windrose-auth.db"))
}

fn hash_password(password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon = Argon2::default();
    argon
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| format!("Password hash failed: {e}"))
}

fn verify_password(password: &str, expected_hash: &str) -> Result<bool, String> {
    let parsed = PasswordHash::new(expected_hash)
        .map_err(|e| format!("Invalid stored password hash: {e}"))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

fn random_token(len: usize) -> String {
    OsRng
        .sample_iter(&Alphanumeric)
        .take(len)
        .map(char::from)
        .collect()
}

fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

#[allow(dead_code)]
fn _ensure_auth_path_is_absolute(path: &Path) -> bool {
    path.is_absolute()
}
