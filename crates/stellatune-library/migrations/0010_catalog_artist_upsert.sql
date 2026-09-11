-- The outer track UPSERT can override INSERT OR IGNORE in a trigger.
-- Deduplicate track artists and album artist before inserting either set.
DROP TRIGGER catalog_artist_insert;
CREATE TRIGGER catalog_artist_insert AFTER INSERT ON tracks BEGIN
  INSERT INTO catalog_artists
    SELECT t.id,j.value FROM catalog_tracks t,json_each(t.artist_names) j WHERE t.id=new.id
    UNION SELECT new.id,trim(new.album_artist) WHERE trim(coalesce(new.album_artist,''))<>'';
END;

DROP TRIGGER catalog_artist_update;
CREATE TRIGGER catalog_artist_update AFTER UPDATE OF artist,album_artist,artists_json ON tracks BEGIN
  DELETE FROM catalog_artists WHERE track_id=new.id;
  INSERT INTO catalog_artists
    SELECT t.id,j.value FROM catalog_tracks t,json_each(t.artist_names) j WHERE t.id=new.id
    UNION SELECT new.id,trim(new.album_artist) WHERE trim(coalesce(new.album_artist,''))<>'';
END;
