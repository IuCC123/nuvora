-- Nuvora schema, migration 6: subtitle preferences per user.
-- subtitle_mode: 'auto' | 'always' | 'off', parsed by `accounts::SubtitleMode`.
-- subtitle_language: ISO 639-2 ('spa'); NULL means the device's language.
ALTER TABLE user ADD COLUMN subtitle_mode TEXT NOT NULL DEFAULT 'auto';
ALTER TABLE user ADD COLUMN subtitle_language TEXT;
