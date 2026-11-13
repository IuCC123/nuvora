-- Genres as an index: one row per (title, genre), keyed by a normalized
-- genre so "Science-Fiction" (TVmaze), "Sci-Fi" (AniList) and "science
-- fiction film" (Wikidata) are one genre. Derived from item_metadata.genres
-- by the library (`genres.rs`), which rebuilds it whenever
-- catalog.genre_index is older than its normalization rules.
CREATE TABLE item_genre (
    item_id INTEGER NOT NULL REFERENCES item(id) ON DELETE CASCADE,
    genre   TEXT    NOT NULL,
    PRIMARY KEY (item_id, genre)
) STRICT, WITHOUT ROWID;
CREATE INDEX item_genre_genre ON item_genre (genre, item_id);

ALTER TABLE catalog ADD COLUMN genre_index INTEGER NOT NULL DEFAULT 0;
