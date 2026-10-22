-- Subtitle files next to a video (`Movie (2010).en.srt`, `Subs/2_English.srt`),
-- found by the scan. What the name says about the track is stored, so item
-- pages read it without touching the disk.
CREATE TABLE media_sidecar (
    file_id     INTEGER NOT NULL REFERENCES media_file(id) ON DELETE CASCADE,
    rel_path    TEXT    NOT NULL,      -- relative to the video's library root
    size        INTEGER NOT NULL,
    mtime_ns    INTEGER NOT NULL,
    format      TEXT    NOT NULL,      -- 'srt' | 'ass' | 'vtt'
    language    TEXT,
    title       TEXT,
    is_forced   INTEGER NOT NULL,
    is_sdh      INTEGER NOT NULL,
    is_default  INTEGER NOT NULL,
    PRIMARY KEY (file_id, rel_path)
) STRICT, WITHOUT ROWID;
