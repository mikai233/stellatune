PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS scan_roots (
  id INTEGER PRIMARY KEY,
  path TEXT NOT NULL UNIQUE,
  enabled INTEGER NOT NULL DEFAULT 1,
  last_scan_ms INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS tracks (
  id INTEGER PRIMARY KEY,
  path TEXT NOT NULL UNIQUE,
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
  codec TEXT
);

CREATE INDEX IF NOT EXISTS idx_tracks_path ON tracks(path);
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

