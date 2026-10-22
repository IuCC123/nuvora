//! Users, sessions and server settings. The rules that span rows (one first
//! admin, the last enabled admin, nobody disables themselves) are checked
//! inside the write transaction that applies the change, so two racing
//! requests cannot both pass a check.
//!
//! Passwords and tokens arrive already hashed: the server owns the hashing
//! policy, the catalog owns the rows.

use rusqlite::{Connection, OptionalExtension, Transaction, params};

use crate::{Library, QueryError, UserId, WatchRev};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Admin,
    Member,
}

impl Role {
    fn as_str(self) -> &'static str {
        match self {
            Role::Admin => "admin",
            Role::Member => "member",
        }
    }

    fn parse(s: &str) -> Option<Role> {
        match s {
            "admin" => Some(Role::Admin),
            "member" => Some(Role::Member),
            _ => None,
        }
    }
}

/// When the player turns subtitles on for this user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SubtitleMode {
    #[default]
    Auto,
    Always,
    Off,
}

impl SubtitleMode {
    fn as_str(self) -> &'static str {
        match self {
            SubtitleMode::Auto => "auto",
            SubtitleMode::Always => "always",
            SubtitleMode::Off => "off",
        }
    }

    fn parse(s: &str) -> Option<SubtitleMode> {
        match s {
            "auto" => Some(SubtitleMode::Auto),
            "always" => Some(SubtitleMode::Always),
            "off" => Some(SubtitleMode::Off),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    pub id: UserId,
    pub username: String,
    pub display_name: String,
    pub role: Role,
    /// Unix ms.
    pub created_at: i64,
    pub disabled: bool,
    /// Unix ms of the latest request from any of the user's sessions.
    pub last_seen_at: Option<i64>,
    pub subtitle_mode: SubtitleMode,
    /// ISO 639-2; `None` means the device's language.
    pub subtitle_language: Option<String>,
}

/// A user to create. `username` is unique ignoring case.
#[derive(Debug, Clone)]
pub struct NewUser {
    pub username: String,
    pub display_name: String,
    /// An argon2id PHC string.
    pub password_hash: String,
    pub role: Role,
}

/// Fields to change; `None` leaves a field as it is.
#[derive(Debug, Clone, Default)]
pub struct UserChanges {
    pub display_name: Option<String>,
    pub role: Option<Role>,
    pub disabled: Option<bool>,
    pub password_hash: Option<String>,
    pub subtitle_mode: Option<SubtitleMode>,
    /// `Some(None)` clears the language.
    pub subtitle_language: Option<Option<String>>,
}

#[derive(Debug, thiserror::Error)]
pub enum AccountError {
    #[error("setup is already done")]
    SetupDone,
    #[error("that username is taken")]
    UsernameTaken,
    #[error("the change would leave no enabled admin")]
    LastAdmin,
    #[error("you cannot disable or delete your own account")]
    OwnAccount,
    #[error("no such user")]
    NotFound,
    #[error(transparent)]
    Query(#[from] QueryError),
}

impl From<crate::db::DbError> for AccountError {
    fn from(e: crate::db::DbError) -> Self {
        AccountError::Query(QueryError(e))
    }
}

/// SHA-256 of a session token. Only the hash is stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenHash(pub [u8; 32]);

/// A live session: not expired, and its user is not disabled.
#[derive(Debug, Clone)]
pub struct Session {
    pub user: User,
    /// The user's watch revision when the session was read.
    pub watch_rev: WatchRev,
    /// Unix ms.
    pub last_seen_at: i64,
    /// Unix ms.
    pub expires_at: i64,
}

/// A user and their stored password hash, for sign in.
#[derive(Debug, Clone)]
pub struct Credentials {
    pub user: User,
    pub password_hash: String,
}

const SERVER_NAME: &str = "server_name";

const USER_SELECT: &str = "SELECT u.id, u.username, u.display_name, u.role, u.created_at, \
     u.disabled_at IS NOT NULL, \
     (SELECT max(s.last_seen_at) FROM session s WHERE s.user_id = u.id), \
     u.subtitle_mode, u.subtitle_language \
     FROM user u";

fn user_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<User> {
    let role: String = r.get(3)?;
    let subtitle_mode: String = r.get(7)?;
    let unknown = |column: usize, what: &str, value: &str| {
        rusqlite::Error::FromSqlConversionFailure(
            column,
            rusqlite::types::Type::Text,
            format!("unknown {what} {value:?}").into(),
        )
    };
    Ok(User {
        id: UserId(r.get(0)?),
        username: r.get(1)?,
        display_name: r.get(2)?,
        role: Role::parse(&role).ok_or_else(|| unknown(3, "role", &role))?,
        created_at: r.get(4)?,
        disabled: r.get(5)?,
        last_seen_at: r.get(6)?,
        subtitle_mode: SubtitleMode::parse(&subtitle_mode)
            .ok_or_else(|| unknown(7, "subtitle mode", &subtitle_mode))?,
        subtitle_language: r.get(8)?,
    })
}

fn load_user(c: &Connection, id: UserId) -> rusqlite::Result<Option<User>> {
    c.query_row(&format!("{USER_SELECT} WHERE u.id = ?1"), [id.0], user_row)
        .optional()
}

fn insert_user(tx: &Transaction<'_>, new: &NewUser, now_ms: i64) -> rusqlite::Result<Option<User>> {
    let taken = tx
        .query_row(
            "SELECT 1 FROM user WHERE username = ?1",
            [&new.username],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    if taken {
        return Ok(None);
    }
    tx.execute(
        "INSERT INTO user (username, display_name, password_hash, role, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            new.username,
            new.display_name,
            new.password_hash,
            new.role.as_str(),
            now_ms
        ],
    )?;
    load_user(tx, UserId(tx.last_insert_rowid()))
}

/// Enabled admins other than `except`.
fn other_enabled_admins(tx: &Transaction<'_>, except: UserId) -> rusqlite::Result<i64> {
    tx.query_row(
        "SELECT count(*) FROM user WHERE role = 'admin' AND disabled_at IS NULL AND id != ?1",
        [except.0],
        |r| r.get(0),
    )
}

impl Library {
    /// True while no user exists: the first request must create the admin.
    pub async fn setup_required(&self) -> Result<bool, QueryError> {
        Ok(self
            .inner
            .db
            .read(|c| c.query_row("SELECT NOT EXISTS (SELECT 1 FROM user)", [], |r| r.get(0)))
            .await?)
    }

    /// Creates the first user, an admin regardless of `new.role`. The zero-user
    /// check and the insert share one write transaction, so of two racing
    /// calls exactly one succeeds and the other gets [`AccountError::SetupDone`].
    pub async fn create_first_admin(
        &self,
        new: NewUser,
        now_ms: i64,
    ) -> Result<User, AccountError> {
        let new = NewUser {
            role: Role::Admin,
            ..new
        };
        self.inner
            .db
            .write(move |tx| {
                let any: bool =
                    tx.query_row("SELECT EXISTS (SELECT 1 FROM user)", [], |r| r.get(0))?;
                if any {
                    return Ok(Err(AccountError::SetupDone));
                }
                Ok(insert_user(tx, &new, now_ms)?.ok_or(AccountError::UsernameTaken))
            })
            .await?
    }

    pub async fn create_user(&self, new: NewUser, now_ms: i64) -> Result<User, AccountError> {
        self.inner
            .db
            .write(move |tx| Ok(insert_user(tx, &new, now_ms)?.ok_or(AccountError::UsernameTaken)))
            .await?
    }

    /// The user named `username` (ignoring case) with their password hash,
    /// disabled or not; the caller decides what a disabled user may do.
    pub async fn credentials(&self, username: String) -> Result<Option<Credentials>, QueryError> {
        Ok(self
            .inner
            .db
            .read(move |c| {
                let Some(user) = c
                    .query_row(
                        &format!("{USER_SELECT} WHERE u.username = ?1"),
                        [&username],
                        user_row,
                    )
                    .optional()?
                else {
                    return Ok(None);
                };
                let password_hash = c.query_row(
                    "SELECT password_hash FROM user WHERE id = ?1",
                    [user.id.0],
                    |r| r.get(0),
                )?;
                Ok(Some(Credentials {
                    user,
                    password_hash,
                }))
            })
            .await?)
    }

    /// The stored password hash of `id`, for checking a current password.
    pub async fn password_hash(&self, id: UserId) -> Result<Option<String>, QueryError> {
        Ok(self
            .inner
            .db
            .read(move |c| {
                c.query_row(
                    "SELECT password_hash FROM user WHERE id = ?1",
                    [id.0],
                    |r| r.get(0),
                )
                .optional()
            })
            .await?)
    }

    /// Every user, oldest first.
    pub async fn users(&self) -> Result<Vec<User>, QueryError> {
        Ok(self
            .inner
            .db
            .read(|c| {
                c.prepare(&format!("{USER_SELECT} ORDER BY u.id"))?
                    .query_map([], user_row)?
                    .collect()
            })
            .await?)
    }

    pub async fn user(&self, id: UserId) -> Result<Option<User>, QueryError> {
        Ok(self.inner.db.read(move |c| load_user(c, id)).await?)
    }

    /// Applies `changes` to `id` on behalf of `actor`. Refuses to disable
    /// `actor` itself, and to demote or disable the last enabled admin.
    /// Disabling deletes every session of the user. A new password deletes
    /// every session of the user except `keep` (the caller's own).
    pub async fn update_user(
        &self,
        actor: UserId,
        id: UserId,
        changes: UserChanges,
        keep: Option<TokenHash>,
        now_ms: i64,
    ) -> Result<User, AccountError> {
        self.inner
            .db
            .write(move |tx| {
                let Some(user) = load_user(tx, id)? else {
                    return Ok(Err(AccountError::NotFound));
                };
                if changes.disabled == Some(true) && actor == id {
                    return Ok(Err(AccountError::OwnAccount));
                }
                let loses_admin =
                    changes.role == Some(Role::Member) || changes.disabled == Some(true);
                if user.role == Role::Admin
                    && !user.disabled
                    && loses_admin
                    && other_enabled_admins(tx, id)? == 0
                {
                    return Ok(Err(AccountError::LastAdmin));
                }
                if let Some(name) = &changes.display_name {
                    tx.execute(
                        "UPDATE user SET display_name = ?1 WHERE id = ?2",
                        params![name, id.0],
                    )?;
                }
                if let Some(role) = changes.role {
                    tx.execute(
                        "UPDATE user SET role = ?1 WHERE id = ?2",
                        params![role.as_str(), id.0],
                    )?;
                }
                match changes.disabled {
                    Some(true) => {
                        tx.execute(
                            "UPDATE user SET disabled_at = coalesce(disabled_at, ?1) WHERE id = ?2",
                            params![now_ms, id.0],
                        )?;
                        tx.execute("DELETE FROM session WHERE user_id = ?1", [id.0])?;
                    }
                    Some(false) => {
                        tx.execute("UPDATE user SET disabled_at = NULL WHERE id = ?1", [id.0])?;
                    }
                    None => {}
                }
                if let Some(mode) = changes.subtitle_mode {
                    tx.execute(
                        "UPDATE user SET subtitle_mode = ?1 WHERE id = ?2",
                        params![mode.as_str(), id.0],
                    )?;
                }
                if let Some(language) = &changes.subtitle_language {
                    tx.execute(
                        "UPDATE user SET subtitle_language = ?1 WHERE id = ?2",
                        params![language, id.0],
                    )?;
                }
                if let Some(hash) = &changes.password_hash {
                    tx.execute(
                        "UPDATE user SET password_hash = ?1 WHERE id = ?2",
                        params![hash, id.0],
                    )?;
                    let keep = keep.map(|k| k.0.to_vec()).unwrap_or_default();
                    tx.execute(
                        "DELETE FROM session WHERE user_id = ?1 AND token_hash != ?2",
                        params![id.0, keep],
                    )?;
                }
                Ok(load_user(tx, id)?.ok_or(AccountError::NotFound))
            })
            .await?
    }

    /// Deletes `id` and its sessions. Refuses `actor` itself and the last
    /// enabled admin.
    pub async fn delete_user(&self, actor: UserId, id: UserId) -> Result<(), AccountError> {
        self.inner
            .db
            .write(move |tx| {
                let Some(user) = load_user(tx, id)? else {
                    return Ok(Err(AccountError::NotFound));
                };
                if actor == id {
                    return Ok(Err(AccountError::OwnAccount));
                }
                if user.role == Role::Admin && !user.disabled && other_enabled_admins(tx, id)? == 0
                {
                    return Ok(Err(AccountError::LastAdmin));
                }
                tx.execute("DELETE FROM user WHERE id = ?1", [id.0])?;
                Ok(Ok(()))
            })
            .await?
    }

    /// Stores a new session and drops every expired one.
    pub async fn create_session(
        &self,
        user: UserId,
        token: TokenHash,
        now_ms: i64,
        expires_at: i64,
        user_agent: Option<String>,
    ) -> Result<(), QueryError> {
        Ok(self
            .inner
            .db
            .write(move |tx| {
                tx.execute("DELETE FROM session WHERE expires_at <= ?1", [now_ms])?;
                tx.execute(
                    "INSERT INTO session (token_hash, user_id, created_at, last_seen_at, expires_at, user_agent) \
                     VALUES (?1, ?2, ?3, ?3, ?4, ?5)",
                    params![token.0.to_vec(), user.0, now_ms, expires_at, user_agent],
                )?;
                Ok(())
            })
            .await?)
    }

    /// The session for `token` if it has not expired by `now_ms` and its user
    /// is enabled. A read: no write per request.
    pub async fn session(
        &self,
        token: TokenHash,
        now_ms: i64,
    ) -> Result<Option<Session>, QueryError> {
        Ok(self
            .inner
            .db
            .read(move |c| {
                let found: Option<(i64, i64, i64, i64)> = c
                    .query_row(
                        "SELECT s.user_id, s.last_seen_at, s.expires_at, u.watch_rev FROM session s \
                         JOIN user u ON u.id = s.user_id \
                         WHERE s.token_hash = ?1 AND s.expires_at > ?2 AND u.disabled_at IS NULL",
                        params![token.0.to_vec(), now_ms],
                        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                    )
                    .optional()?;
                let Some((user_id, last_seen_at, expires_at, watch_rev)) = found else {
                    return Ok(None);
                };
                Ok(load_user(c, UserId(user_id))?.map(|user| Session {
                    user,
                    watch_rev: WatchRev(watch_rev as u64),
                    last_seen_at,
                    expires_at,
                }))
            })
            .await?)
    }

    /// Records activity on a session, and moves its expiry when `expires_at`
    /// is given.
    pub async fn touch_session(
        &self,
        token: TokenHash,
        now_ms: i64,
        expires_at: Option<i64>,
    ) -> Result<(), QueryError> {
        Ok(self
            .inner
            .db
            .write(move |tx| {
                tx.execute(
                    "UPDATE session SET last_seen_at = ?2, expires_at = coalesce(?3, expires_at) \
                     WHERE token_hash = ?1",
                    params![token.0.to_vec(), now_ms, expires_at],
                )?;
                Ok(())
            })
            .await?)
    }

    pub async fn delete_session(&self, token: TokenHash) -> Result<(), QueryError> {
        Ok(self
            .inner
            .db
            .write(move |tx| {
                tx.execute(
                    "DELETE FROM session WHERE token_hash = ?1",
                    [token.0.to_vec()],
                )?;
                Ok(())
            })
            .await?)
    }

    /// The server name set in the admin panel, if any.
    pub async fn server_name(&self) -> Result<Option<String>, QueryError> {
        Ok(self
            .inner
            .db
            .read(|c| {
                c.query_row(
                    "SELECT value FROM setting WHERE key = ?1",
                    [SERVER_NAME],
                    |r| r.get(0),
                )
                .optional()
            })
            .await?)
    }

    pub async fn set_server_name(&self, name: String) -> Result<(), QueryError> {
        Ok(self
            .inner
            .db
            .write(move |tx| {
                tx.execute(
                    "INSERT INTO setting (key, value) VALUES (?1, ?2) \
                     ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                    params![SERVER_NAME, name],
                )?;
                Ok(())
            })
            .await?)
    }
}
