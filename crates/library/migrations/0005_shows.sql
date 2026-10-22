-- Nuvora schema, migration 5: shows (library.kind = 'shows').
--
-- parse_version: the filename parser version (nuvora_media::PARSE_VERSION)
-- that placed the file in its item. Existing rows start at 0, so the next
-- scan places every file once more with the current parser.
ALTER TABLE media_file ADD COLUMN parse_version INTEGER NOT NULL DEFAULT 0;

-- Seasons of a show and episodes of a season, in order.
DROP INDEX item_parent;
CREATE INDEX item_children ON item (parent_id, index_number) WHERE parent_id IS NOT NULL;
