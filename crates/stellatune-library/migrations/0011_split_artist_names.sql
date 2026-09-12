-- Preserve the original credits/album identity while indexing individual names.
ALTER TABLE tracks ADD COLUMN album_artists_json TEXT NOT NULL DEFAULT '[]';
ALTER TABLE tracks ADD COLUMN artist_names_version INTEGER NOT NULL DEFAULT 0;
CREATE INDEX idx_artist_names_backfill ON tracks(id) WHERE artist_names_version=0;

-- Rust backfills existing rows using the same parser as scans and file watching.
-- UNION deduplicates names before insertion, including outer UPSERT statements.
DROP TRIGGER catalog_artist_insert;
CREATE TRIGGER catalog_artist_insert AFTER INSERT ON tracks BEGIN
  INSERT INTO catalog_artists
    SELECT t.id,j.value FROM catalog_tracks t,json_each(t.artist_names) j WHERE t.id=new.id
    UNION
    SELECT new.id,j.value FROM json_each(
      CASE WHEN json_array_length(new.album_artists_json)>0 THEN new.album_artists_json
      ELSE json_array(trim(coalesce(new.album_artist,''))) END
    ) j WHERE j.value<>'';
END;

DROP TRIGGER catalog_artist_update;
CREATE TRIGGER catalog_artist_update AFTER UPDATE OF artist,album_artist,artists_json,album_artists_json ON tracks BEGIN
  DELETE FROM catalog_artists WHERE track_id=new.id;
  INSERT INTO catalog_artists
    SELECT t.id,j.value FROM catalog_tracks t,json_each(t.artist_names) j WHERE t.id=new.id
    UNION
    SELECT new.id,j.value FROM json_each(
      CASE WHEN json_array_length(new.album_artists_json)>0 THEN new.album_artists_json
      ELSE json_array(trim(coalesce(new.album_artist,''))) END
    ) j WHERE j.value<>'';
END;
