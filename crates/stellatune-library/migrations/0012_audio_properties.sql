-- Additive technical cache upgrade; never replaces tracks, favorites or playlists.
ALTER TABLE audio_files ADD COLUMN properties_json TEXT;
ALTER TABLE audio_files ADD COLUMN probe_version INTEGER NOT NULL DEFAULT 0;
ALTER TABLE audio_files ADD COLUMN probe_status TEXT;
ALTER TABLE audio_files ADD COLUMN probe_mtime_ms INTEGER;
ALTER TABLE audio_files ADD COLUMN probe_size_bytes INTEGER;

CREATE TRIGGER catalog_audio_properties_au AFTER UPDATE OF properties_json ON audio_files
WHEN old.properties_json IS NOT new.properties_json
BEGIN
  UPDATE catalog_revision SET revision=revision+1;
END;
