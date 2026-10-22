-- Nuvora schema, migration 2: accounts, sessions, server settings.

-- role: 'admin' | 'member', parsed by `accounts::Role`. password_hash is an
-- argon2id PHC string; the catalog stores it and never interprets it.
CREATE TABLE user (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    username      TEXT    NOT NULL UNIQUE COLLATE NOCASE,
    display_name  TEXT    NOT NULL,
    password_hash TEXT    NOT NULL,
    role          TEXT    NOT NULL,
    disabled_at   INTEGER,                   -- unix ms or NULL
    created_at    INTEGER NOT NULL           -- unix ms
) STRICT;

-- token_hash is SHA-256 of the 32-byte token the client holds, so a copy of
-- the database cannot be used to sign in. Times are unix ms.
CREATE TABLE session (
    token_hash   BLOB    PRIMARY KEY,
    user_id      INTEGER NOT NULL REFERENCES user(id) ON DELETE CASCADE,
    created_at   INTEGER NOT NULL,
    last_seen_at INTEGER NOT NULL,
    expires_at   INTEGER NOT NULL,
    user_agent   TEXT
) STRICT, WITHOUT ROWID;
CREATE INDEX session_user ON session (user_id);

CREATE TABLE setting (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
) STRICT, WITHOUT ROWID;
