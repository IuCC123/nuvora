-- Markers found by comparing a show's episodes by sound (source 'audio'),
-- beside the ones chapters name. A probe rewrites only a file's chapter
-- markers; where both name the same kind, the chapter's wins.
ALTER TABLE media_marker ADD COLUMN source TEXT NOT NULL DEFAULT 'chapters';

-- What a show's audio markers were found from: a hash of its episode files
-- and their content. When that changes (an episode added, replaced or
-- removed), the show is looked at again.
CREATE TABLE audio_marker_show (
    show_id   INTEGER PRIMARY KEY REFERENCES item(id) ON DELETE CASCADE,
    signature BLOB NOT NULL
) STRICT;
