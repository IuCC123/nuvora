-- Nuvora schema, migration 4: the Dolby Vision base-layer compatibility id
-- (`dv_bl_signal_compatibility_id`), so whether a stream has an HDR10 base
-- layer is stored instead of guessed from the profile. PROBE_VERSION 2
-- re-probes every file once to fill it.
ALTER TABLE media_stream ADD COLUMN dv_compatibility INTEGER;
