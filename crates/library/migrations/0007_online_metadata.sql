-- Online metadata: descriptions, genres, ratings, cast and voice actors from
-- AniList (anime), TVmaze (TV) and TMDB (movies, with an API key). Written
-- only by the metadata job, as the `online` layer of item_metadata; artwork
-- it downloads is `artwork.source = 'online'`, which item_artwork already
-- ranks between local files and frame grabs.

ALTER TABLE item_metadata ADD COLUMN genres TEXT;         -- JSON array of strings
ALTER TABLE item_metadata ADD COLUMN rating REAL;         -- 0..10
ALTER TABLE item_metadata ADD COLUMN studio TEXT;
ALTER TABLE item_metadata ADD COLUMN external_url TEXT;   -- the provider's page

-- Precedence: user > online > filename. Identity never follows this view.
DROP VIEW item_display;
CREATE VIEW item_display AS
SELECT i.id, i.library_id, i.kind, i.parent_id, i.index_number, i.added_at, i.missing_since,
       COALESCE(u.title, o.title, f.title) AS title,
       COALESCE(u.sort_title, o.sort_title, u.title, o.title, f.sort_title, f.title) AS sort_title,
       COALESCE(u.year, o.year, f.year) AS year,
       COALESCE(u.overview, o.overview, f.overview) AS overview,
       COALESCE(u.tagline, o.tagline, f.tagline) AS tagline
FROM item i
LEFT JOIN item_metadata f ON f.item_id = i.id AND f.source = 'filename'
LEFT JOIN item_metadata o ON o.item_id = i.id AND o.source = 'online'
LEFT JOIN item_metadata u ON u.item_id = i.id AND u.source = 'user';

-- Cast and crew in billing order. `kind` is 'cast' (an actor and the
-- character they play) or 'voice' (a voice actor; `language` says which dub).
-- Images are ArtworkIds in the artwork store; live_artwork_ids keeps them.
CREATE TABLE item_credit (
    item_id         INTEGER NOT NULL REFERENCES item(id) ON DELETE CASCADE,
    ord             INTEGER NOT NULL,
    kind            TEXT    NOT NULL,
    name            TEXT    NOT NULL,
    character       TEXT,
    language        TEXT,
    image           BLOB,
    image_aspect    REAL,
    image_ph        BLOB,
    character_image BLOB,
    character_aspect REAL,
    character_ph    BLOB,
    PRIMARY KEY (item_id, ord)
) STRICT, WITHOUT ROWID;

-- One row per item the job has looked up, so a failed or empty lookup is not
-- repeated on every scan. Deleting the row (Refresh metadata) retries it.
CREATE TABLE metadata_lookup (
    item_id     INTEGER PRIMARY KEY REFERENCES item(id) ON DELETE CASCADE,
    looked_at   INTEGER NOT NULL,
    outcome     TEXT    NOT NULL       -- 'matched' | 'no_match' | 'no_provider' | 'error'
) STRICT;
