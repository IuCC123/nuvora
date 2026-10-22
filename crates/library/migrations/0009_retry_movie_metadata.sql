-- Movies now have a metadata provider that needs no key (Wikidata with
-- Wikipedia's summaries), so look up again every movie that found nothing.
DELETE FROM metadata_lookup
WHERE outcome IN ('no_provider', 'no_match')
  AND item_id IN (SELECT id FROM item WHERE kind = 'movie');
