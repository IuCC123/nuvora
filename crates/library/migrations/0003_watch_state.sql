-- Nuvora schema, migration 3: per-user watch state.

-- One row per (user, item) the user has touched. Resume columns (file_id,
-- position_ms, duration_ms) are all set or all NULL: set only while the item is
-- in progress. file_id has no foreign key: a version can disappear while its
-- position stays useful for the item's other versions.
CREATE TABLE watch_state (
    user_id         INTEGER NOT NULL REFERENCES user(id) ON DELETE CASCADE,
    item_id         INTEGER NOT NULL REFERENCES item(id) ON DELETE CASCADE,
    file_id         INTEGER,
    position_ms     INTEGER,
    duration_ms     INTEGER,
    play_count      INTEGER NOT NULL DEFAULT 0,
    last_watched_at INTEGER,                 -- unix ms of the last finish
    updated_at      INTEGER NOT NULL,        -- unix ms
    PRIMARY KEY (user_id, item_id)
) STRICT, WITHOUT ROWID;
CREATE INDEX watch_state_continue ON watch_state (user_id, updated_at DESC)
    WHERE position_ms IS NOT NULL;
CREATE INDEX watch_state_item ON watch_state (item_id);

-- Bumped in the same transaction as every watch_state write of the user. It is
-- half of the user's home, row and item ETags. Values come from
-- catalog.watch_seq, one sequence for every user, so two users share a
-- watch_rev only while both are 0 (no state at all): a browser shared by two
-- users can never revalidate one user's cached page as the other's.
ALTER TABLE user ADD COLUMN watch_rev INTEGER NOT NULL DEFAULT 0;
ALTER TABLE catalog ADD COLUMN watch_seq INTEGER NOT NULL DEFAULT 0;
