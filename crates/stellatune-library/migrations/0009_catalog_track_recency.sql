CREATE INDEX idx_catalog_track_recency ON tracks(mtime_ms DESC, id DESC);
CREATE INDEX idx_catalog_folder_recency ON tracks(dir_norm, mtime_ms DESC, id DESC);
