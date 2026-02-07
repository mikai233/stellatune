ALTER TABLE playlist_tracks
ADD COLUMN sort_index INTEGER NOT NULL DEFAULT 0;

WITH ranked AS (
  SELECT
    playlist_id,
    track_id,
    ROW_NUMBER() OVER (
      PARTITION BY playlist_id
      ORDER BY track_id ASC
    ) - 1 AS rank_index
  FROM playlist_tracks
)
UPDATE playlist_tracks
SET sort_index = (
  SELECT rank_index
  FROM ranked
  WHERE ranked.playlist_id = playlist_tracks.playlist_id
    AND ranked.track_id = playlist_tracks.track_id
);

CREATE INDEX IF NOT EXISTS idx_playlist_tracks_order
ON playlist_tracks(playlist_id, sort_index, track_id);
