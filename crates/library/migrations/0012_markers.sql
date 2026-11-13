-- Skippable stretches of a file (recap, intro, credits) from its named
-- chapters, written with its streams on every probe. PROBE_VERSION 3 makes
-- the next scan read them for files probed before.
CREATE TABLE media_marker (
    file_id  INTEGER NOT NULL REFERENCES media_file(id) ON DELETE CASCADE,
    kind     TEXT    NOT NULL,   -- 'recap' | 'intro' | 'credits'
    start_ms INTEGER NOT NULL,
    end_ms   INTEGER NOT NULL,
    PRIMARY KEY (file_id, start_ms)
) STRICT, WITHOUT ROWID;
