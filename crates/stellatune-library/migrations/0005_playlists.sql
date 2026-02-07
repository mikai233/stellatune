CREATE TABLE IF NOT EXISTS playlists (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  system_key TEXT UNIQUE
);

CREATE TABLE IF NOT EXISTS playlist_tracks (
  playlist_id INTEGER NOT NULL,
  track_id INTEGER NOT NULL,
  PRIMARY KEY (playlist_id, track_id),
  FOREIGN KEY (playlist_id) REFERENCES playlists(id) ON DELETE CASCADE,
  FOREIGN KEY (track_id) REFERENCES tracks(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_playlist_tracks_track ON playlist_tracks(track_id);

INSERT INTO playlists(name, system_key)
VALUES ('我喜欢的音乐', 'liked')
ON CONFLICT(system_key) DO NOTHING;

CREATE TRIGGER IF NOT EXISTS playlists_block_delete_system
BEFORE DELETE ON playlists
FOR EACH ROW
WHEN old.system_key IS NOT NULL
BEGIN
  SELECT RAISE(ABORT, 'cannot delete system playlist');
END;
