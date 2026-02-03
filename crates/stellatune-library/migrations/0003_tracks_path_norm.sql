ALTER TABLE tracks
ADD COLUMN path_norm TEXT NOT NULL DEFAULT '';

ALTER TABLE tracks
ADD COLUMN dir_norm TEXT NOT NULL DEFAULT '';

CREATE INDEX IF NOT EXISTS idx_tracks_dir_norm ON tracks(dir_norm);
CREATE INDEX IF NOT EXISTS idx_tracks_path_norm ON tracks(path_norm);

