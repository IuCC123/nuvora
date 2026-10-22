-- Nuvora schema, migration 1. Applied by `db::migrate` inside one transaction,
-- tracked with PRAGMA user_version. Never edit an applied migration; add 0002_*.sql.
--
-- Ownership: every table here is written only by the `nuvora-library` crate.
-- Kinds and enums (item.kind, media_stream.kind, codecs) carry no CHECK lists:
-- the Rust enums are the single source of truth and parse at the row boundary,
-- so adding `show`/`season`/`episode` later is a Rust change, not a table rebuild.

-- A library is a typed collection of root folders. Step 1: kind = 'movies'.
-- `key` is the stable slug from the config file; the row id never changes for a key.
CREATE TABLE library (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    key         TEXT    NOT NULL UNIQUE,
    name        TEXT    NOT NULL,
    kind        TEXT    NOT NULL            -- 'movies' (Step 2: 'shows')
) STRICT;

-- One configured folder of a library. Files store paths relative to their root,
-- so a remounted NAS share only changes `path` here.
CREATE TABLE library_root (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    library_id  INTEGER NOT NULL REFERENCES library(id) ON DELETE CASCADE,
    path        TEXT    NOT NULL,
    UNIQUE (library_id, path)
) STRICT;

-- A browsable thing: a movie now; show, season and episode later (parent_id,
-- index_number). AUTOINCREMENT so a deleted id is never reused: a client cache
-- holding a stale id can never resolve to a different movie.
--
-- identity_key groups files into one item: casefold(title)|year from the
-- filename parser. Two files with the same key are two versions of one movie.
-- It is built from the filename layer only and never rewritten from another
-- source, not by a TMDB match and not by a user fix, so a re-probe re-derives
-- the same key and can never split a matched item into a duplicate. A TMDB match
-- writes item_metadata.provider_id on the 'tmdb' row instead. The one change is
-- a rename whose new name parses differently (scan/write.rs `move_file`).
--
-- missing_since: set when the last file of the item disappears. The row (and so
-- the id, and later the per-user watch state) survives; the home query hides it.
-- A returning file with the same identity_key clears it.
CREATE TABLE item (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    library_id    INTEGER NOT NULL REFERENCES library(id) ON DELETE CASCADE,
    kind          TEXT    NOT NULL,          -- 'movie' (Step 2: 'show','season','episode')
    parent_id     INTEGER REFERENCES item(id) ON DELETE CASCADE,  -- season->show, episode->season
    index_number  INTEGER,                   -- season/episode number; NULL for movies
    identity_key  TEXT    NOT NULL,
    added_at      INTEGER NOT NULL,          -- unix ms, first time any file of this item was seen
    missing_since INTEGER,                   -- unix ms or NULL
    UNIQUE (library_id, identity_key)
) STRICT;
CREATE INDEX item_recent ON item (library_id, added_at DESC) WHERE missing_since IS NULL;
CREATE INDEX item_parent ON item (parent_id) WHERE parent_id IS NOT NULL;

-- Descriptive metadata in layers, one row per (item, source). Step 1 writes only
-- source = 'filename'. Step 2 adds 'tmdb' rows and 'user' rows (sparse: only the
-- fields the user fixed, plus the pinned provider id). Precedence lives in exactly
-- one place: the item_display view below. Re-matching TMDB rewrites only its layer.
CREATE TABLE item_metadata (
    item_id        INTEGER NOT NULL REFERENCES item(id) ON DELETE CASCADE,
    source         TEXT    NOT NULL,         -- 'filename' | later 'tmdb' | 'user'
    title          TEXT,
    sort_title     TEXT,
    original_title TEXT,
    year           INTEGER,
    overview       TEXT,
    tagline        TEXT,
    provider_id    TEXT,                     -- e.g. 'tmdb:603'; NULL for filename
    updated_at     INTEGER NOT NULL,
    PRIMARY KEY (item_id, source)
) STRICT, WITHOUT ROWID;

-- Effective metadata. Step 2 redefines this view to COALESCE user > tmdb > filename;
-- no table changes. Readers only ever select from the view.
CREATE VIEW item_display AS
SELECT i.id, i.library_id, i.kind, i.parent_id, i.index_number, i.added_at, i.missing_since,
       f.title, COALESCE(f.sort_title, f.title) AS sort_title, f.year, f.overview, f.tagline
FROM item i
LEFT JOIN item_metadata f ON f.item_id = i.id AND f.source = 'filename';

-- One file on disk = one version of an item.
-- Identity: (root_id, rel_path). Change detection: (size, mtime_ns). Rename/move
-- detection: fingerprint = blake3(size || first 64 KiB || last 64 KiB), matched
-- between files that vanished and files that appeared in the same scan.
-- probe_version: bumping PROBE_VERSION in code re-probes every file once.
-- probe_error: set when ffprobe failed; the file is not retried until it changes.
CREATE TABLE media_file (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    item_id       INTEGER NOT NULL REFERENCES item(id),
    root_id       INTEGER NOT NULL REFERENCES library_root(id) ON DELETE CASCADE,
    rel_path      TEXT    NOT NULL,          -- '/'-separated, relative to root
    size          INTEGER NOT NULL,
    mtime_ns      INTEGER NOT NULL,
    fingerprint   BLOB    NOT NULL,          -- 16 bytes
    added_at      INTEGER NOT NULL,
    probe_version INTEGER NOT NULL,
    probe_error   TEXT,
    container     TEXT,                      -- domain Container::as_str
    duration_ms   INTEGER,
    bitrate       INTEGER,
    UNIQUE (root_id, rel_path)
) STRICT;
CREATE INDEX media_file_item ON media_file (item_id);
CREATE INDEX media_file_fingerprint ON media_file (size, fingerprint);

-- Every stream of a probed file, in ffprobe index order. Replaced wholesale when
-- the file is re-probed (DELETE + INSERT in the same transaction as the file row).
CREATE TABLE media_stream (
    file_id         INTEGER NOT NULL REFERENCES media_file(id) ON DELETE CASCADE,
    stream_index    INTEGER NOT NULL,
    kind            TEXT    NOT NULL,        -- 'video' | 'audio' | 'subtitle'
    codec           TEXT    NOT NULL,        -- domain codec as_str, e.g. 'hevc', 'eac3', 'pgs'
    profile         TEXT,
    level           INTEGER,
    bit_depth       INTEGER,
    width           INTEGER,
    height          INTEGER,
    frame_rate      REAL,
    hdr             TEXT,                    -- 'sdr' | 'hdr10' | 'hlg' | 'dolby_vision' (video only)
    dv_profile      INTEGER,
    color_primaries TEXT,
    color_transfer  TEXT,
    color_space     TEXT,
    channels        INTEGER,
    channel_layout  TEXT,
    sample_rate     INTEGER,
    language        TEXT,                    -- ISO 639-2 as reported, lowercased
    title           TEXT,
    is_default      INTEGER NOT NULL DEFAULT 0,
    is_forced       INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (file_id, stream_index)
) STRICT, WITHOUT ROWID;

-- Artwork per (item, role, source). The image bytes live in the content-addressed
-- store at <data>/artwork/<hh>/<hash>/{small,large}.webp; this row is the only
-- reference to a hash, so store GC = "hashes not in this table".
-- source_sig: what the art was rendered from ('local:<rel_path>:<size>:<mtime_ns>'
-- or 'frame:<file_id>:<fingerprint hex>'); equal sig => skip re-render.
-- Step 2 adds source = 'tmdb' and 'user' rows; precedence lives in item_artwork.
CREATE TABLE artwork (
    item_id     INTEGER NOT NULL REFERENCES item(id) ON DELETE CASCADE,
    role        TEXT    NOT NULL,            -- 'poster' | 'backdrop'
    source      TEXT    NOT NULL,            -- 'local' | 'frame' (backdrops only); later 'tmdb' | 'user'
    source_sig  TEXT    NOT NULL,
    hash        BLOB    NOT NULL,            -- 16 bytes, ArtworkId
    width       INTEGER NOT NULL,            -- of the large variant
    height      INTEGER NOT NULL,
    placeholder BLOB    NOT NULL,            -- ThumbHash bytes (~25)
    PRIMARY KEY (item_id, role, source)
) STRICT, WITHOUT ROWID;

-- The artwork a client sees: best source per (item, role). Step 1 precedence:
-- local > frame. Step 2: user > local > tmdb > frame.
CREATE VIEW item_artwork AS
SELECT a.item_id, a.role, a.hash, a.width, a.height, a.placeholder
FROM artwork a
WHERE a.source = (
    SELECT b.source FROM artwork b
    WHERE b.item_id = a.item_id AND b.role = a.role
    ORDER BY CASE b.source WHEN 'local' THEN 0 WHEN 'frame' THEN 9 ELSE 5 END
    LIMIT 1
);

-- Single row. `rev` is bumped inside every write transaction that changes what
-- a client can see. The API sends it as the ETag of home and row pages, so an
-- unchanged catalog answers a revalidation with 304 and no query.
CREATE TABLE catalog (
    id   INTEGER PRIMARY KEY CHECK (id = 1),
    rev  INTEGER NOT NULL
) STRICT;
INSERT INTO catalog (id, rev) VALUES (1, 0);
