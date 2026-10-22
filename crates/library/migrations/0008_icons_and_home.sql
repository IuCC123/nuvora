-- A preset icon per library (NULL: the default for its kind), and each
-- user's home screen layout as JSON, owned and validated by the API.
ALTER TABLE library ADD COLUMN icon TEXT;

CREATE TABLE user_home (
    user_id    INTEGER PRIMARY KEY REFERENCES user(id) ON DELETE CASCADE,
    layout     TEXT    NOT NULL,
    updated_at INTEGER NOT NULL
) STRICT;
