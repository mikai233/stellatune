PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS scan_roots (
  id INTEGER PRIMARY KEY,
  path TEXT NOT NULL UNIQUE,
  enabled INTEGER NOT NULL DEFAULT 1,
  last_scan_ms INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE library_schema (version INTEGER NOT NULL);
INSERT INTO library_schema VALUES (2);

CREATE TABLE audio_files (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  path TEXT NOT NULL,
  path_norm TEXT NOT NULL UNIQUE,
  ext TEXT NOT NULL DEFAULT '',
  mtime_ms INTEGER NOT NULL,
  size_bytes INTEGER NOT NULL,
  meta_scanned_ms INTEGER NOT NULL DEFAULT 0,
  sample_rate INTEGER,
  total_frames INTEGER,
  pcm_bits INTEGER,
  pcm_float INTEGER NOT NULL DEFAULT 0,
  metadata_json TEXT,
  cover_key INTEGER
);

CREATE TABLE cue_documents (
  path TEXT PRIMARY KEY,
  fingerprint TEXT NOT NULL,
  signature TEXT NOT NULL,
  document_json TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS tracks (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  path TEXT NOT NULL,
  track_key TEXT UNIQUE,
  file_id INTEGER REFERENCES audio_files(id) ON DELETE CASCADE,
  cue_path TEXT REFERENCES cue_documents(path) ON DELETE CASCADE,
  cue_track_number INTEGER,
  start_cd_frame INTEGER,
  end_cd_frame INTEGER,
  start_frame INTEGER,
  end_frame INTEGER,
  segment_sample_rate INTEGER,
  ext TEXT NOT NULL DEFAULT '',
  mtime_ms INTEGER NOT NULL,
  size_bytes INTEGER NOT NULL,

  -- Metadata (filled later)
  title TEXT,
  artist TEXT,
  album TEXT,
  album_artist TEXT,
  duration_ms INTEGER,
  sample_rate INTEGER,
  channels INTEGER,
  codec TEXT,
  CHECK ((start_frame IS NULL AND end_frame IS NULL AND segment_sample_rate IS NULL)
    OR (start_frame >= 0 AND end_frame > start_frame AND segment_sample_rate > 0))
);

CREATE INDEX IF NOT EXISTS idx_tracks_path ON tracks(path);
CREATE INDEX idx_tracks_file ON tracks(file_id);
CREATE INDEX idx_tracks_cue ON tracks(cue_path);
CREATE INDEX IF NOT EXISTS idx_tracks_artist ON tracks(artist);
CREATE INDEX IF NOT EXISTS idx_tracks_album ON tracks(album);

-- Full-text search (FTS5) for common fields. Kept contentless and synchronized via triggers.
CREATE VIRTUAL TABLE IF NOT EXISTS tracks_fts USING fts5(
  title,
  artist,
  album,
  album_artist,
  path,
  tokenize = 'unicode61'
);

CREATE TRIGGER IF NOT EXISTS tracks_ai AFTER INSERT ON tracks BEGIN
  INSERT INTO tracks_fts(rowid, title, artist, album, album_artist, path)
  VALUES (new.id, new.title, new.artist, new.album, new.album_artist, new.path);
END;

CREATE TRIGGER IF NOT EXISTS tracks_ad AFTER DELETE ON tracks BEGIN
  DELETE FROM tracks_fts WHERE rowid = old.id;
END;

CREATE TRIGGER IF NOT EXISTS tracks_au AFTER UPDATE ON tracks BEGIN
  DELETE FROM tracks_fts WHERE rowid = old.id;
  INSERT INTO tracks_fts(rowid, title, artist, album, album_artist, path)
  VALUES (new.id, new.title, new.artist, new.album, new.album_artist, new.path);
END;
