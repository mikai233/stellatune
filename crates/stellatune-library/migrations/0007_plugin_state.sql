CREATE TABLE IF NOT EXISTS plugin_state (
  plugin_id TEXT PRIMARY KEY,
  enabled INTEGER NOT NULL DEFAULT 1,
  install_state TEXT NOT NULL DEFAULT 'installed',
  disable_in_progress INTEGER NOT NULL DEFAULT 0,
  last_error TEXT,
  updated_at_ms INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_plugin_state_enabled
ON plugin_state(enabled);
