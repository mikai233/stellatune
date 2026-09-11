ALTER TABLE tracks ADD COLUMN disc_number INTEGER;
ALTER TABLE tracks ADD COLUMN track_number INTEGER;
ALTER TABLE tracks ADD COLUMN artists_json TEXT NOT NULL DEFAULT '[]';

CREATE INDEX idx_catalog_album ON tracks(album, album_artist, artist, id);
CREATE INDEX idx_catalog_folder ON tracks(dir_norm, id);
CREATE INDEX idx_catalog_album_order ON tracks(album, album_artist, disc_number, track_number, id);

-- Views keep browse results in step with scans, file watching and removals.
CREATE VIEW catalog_tracks AS
SELECT t.*,
  CASE WHEN trim(coalesce(album,'')) = ''
    THEN json_array('', dir_norm)
    ELSE json_array(trim(album), trim(coalesce(nullif(trim(album_artist),''),artist,''))) END AS album_key,
  CASE WHEN json_array_length(artists_json)>0 THEN artists_json
    ELSE json_array(trim(coalesce(artist,''))) END AS artist_names
FROM tracks t;

CREATE INDEX idx_catalog_album_identity ON tracks(
  CASE WHEN trim(coalesce(album,'')) = '' THEN json_array('',dir_norm)
    ELSE json_array(trim(album),trim(coalesce(nullif(trim(album_artist),''),artist,''))) END,
  disc_number, track_number, id
);
CREATE TABLE catalog_artists (
  track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  PRIMARY KEY(track_id,name)
);
CREATE INDEX idx_catalog_artist_tracks ON catalog_artists(name,track_id);
INSERT OR IGNORE INTO catalog_artists
  SELECT t.id,j.value FROM catalog_tracks t,json_each(t.artist_names) j
  UNION SELECT id,trim(album_artist) FROM tracks WHERE trim(coalesce(album_artist,''))<>'';
CREATE TRIGGER catalog_artist_insert AFTER INSERT ON tracks BEGIN
  INSERT OR IGNORE INTO catalog_artists SELECT t.id,j.value FROM catalog_tracks t,json_each(t.artist_names) j WHERE t.id=new.id;
  INSERT OR IGNORE INTO catalog_artists SELECT new.id,trim(new.album_artist) WHERE trim(coalesce(new.album_artist,''))<>'';
END;
CREATE TRIGGER catalog_artist_update AFTER UPDATE OF artist,album_artist,artists_json ON tracks BEGIN
  DELETE FROM catalog_artists WHERE track_id=new.id;
  INSERT OR IGNORE INTO catalog_artists SELECT t.id,j.value FROM catalog_tracks t,json_each(t.artist_names) j WHERE t.id=new.id;
  INSERT OR IGNORE INTO catalog_artists SELECT new.id,trim(new.album_artist) WHERE trim(coalesce(new.album_artist,''))<>'';
END;

CREATE TABLE catalog_revision (singleton INTEGER PRIMARY KEY CHECK(singleton=1), revision INTEGER NOT NULL);
INSERT INTO catalog_revision VALUES(1,0);
CREATE TRIGGER catalog_tracks_ai AFTER INSERT ON tracks BEGIN UPDATE catalog_revision SET revision=revision+1; END;
CREATE TRIGGER catalog_tracks_au AFTER UPDATE ON tracks BEGIN UPDATE catalog_revision SET revision=revision+1; END;
CREATE TRIGGER catalog_tracks_ad AFTER DELETE ON tracks BEGIN UPDATE catalog_revision SET revision=revision+1; END;
CREATE TRIGGER catalog_playlists_ai AFTER INSERT ON playlists BEGIN UPDATE catalog_revision SET revision=revision+1; END;
CREATE TRIGGER catalog_playlists_au AFTER UPDATE ON playlists BEGIN UPDATE catalog_revision SET revision=revision+1; END;
CREATE TRIGGER catalog_playlists_ad AFTER DELETE ON playlists BEGIN UPDATE catalog_revision SET revision=revision+1; END;
CREATE TRIGGER catalog_playlist_tracks_ai AFTER INSERT ON playlist_tracks BEGIN UPDATE catalog_revision SET revision=revision+1; END;
CREATE TRIGGER catalog_playlist_tracks_au AFTER UPDATE ON playlist_tracks BEGIN UPDATE catalog_revision SET revision=revision+1; END;
CREATE TRIGGER catalog_playlist_tracks_ad AFTER DELETE ON playlist_tracks BEGIN UPDATE catalog_revision SET revision=revision+1; END;
CREATE TRIGGER catalog_roots_ai AFTER INSERT ON scan_roots BEGIN UPDATE catalog_revision SET revision=revision+1; END;
CREATE TRIGGER catalog_roots_au AFTER UPDATE ON scan_roots BEGIN UPDATE catalog_revision SET revision=revision+1; END;
CREATE TRIGGER catalog_roots_ad AFTER DELETE ON scan_roots BEGIN UPDATE catalog_revision SET revision=revision+1; END;
