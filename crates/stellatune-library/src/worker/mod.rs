mod db;
mod fts;
mod metadata;
mod paths;
mod scan;
mod tracks;
mod watch;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use sqlx::SqlitePool;
use tokio::sync::mpsc;

use stellatune_core::{LibraryCommand, LibraryEvent, TrackLite};

use crate::service::EventHub;

use self::fts::build_fts_query;
use self::paths::{is_drive_root, normalize_path_str, parent_dir_norm};
use self::watch::{WatchCtrl, spawn_watch_task};

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use std::collections::HashSet;

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use stellatune_plugins::{PluginManager, default_host_vtable};

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub(crate) type Plugins = std::sync::Arc<std::sync::Mutex<PluginManager>>;

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
pub(crate) type Plugins = ();

pub(crate) struct WorkerDeps {
    pool: SqlitePool,
    events: std::sync::Arc<EventHub>,
    cover_dir: PathBuf,
    plugins_dir: PathBuf,
    plugins: Plugins,
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
const DISABLED_PLUGINS_FILE_NAME: &str = "disabled_plugins.json";

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
fn read_disabled_plugin_ids_best_effort(plugins_dir: &Path) -> HashSet<String> {
    let path = plugins_dir.join(DISABLED_PLUGINS_FILE_NAME);
    let text = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return HashSet::new(),
    };

    let ids = match serde_json::from_str::<serde_json::Value>(&text) {
        Ok(v) => v,
        Err(e) => {
            tracing::debug!(
                target: "stellatune_library::plugins",
                file = %path.display(),
                "failed to parse disabled plugin list: {e}"
            );
            return HashSet::new();
        }
    };

    let Some(arr) = ids.as_array() else {
        return HashSet::new();
    };

    arr.iter()
        .filter_map(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect::<HashSet<_>>()
}

impl WorkerDeps {
    pub(crate) async fn new(db_path: &Path, events: std::sync::Arc<EventHub>) -> Result<Self> {
        let pool = db::init_db(db_path).await?;

        let cover_dir = db_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("covers");
        std::fs::create_dir_all(&cover_dir).with_context(|| {
            format!("failed to create cover cache dir: {}", cover_dir.display())
        })?;

        let plugins_dir = db_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("plugins");

        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
        let plugins: Plugins = {
            let plugins = std::sync::Arc::new(std::sync::Mutex::new(PluginManager::new(
                default_host_vtable(),
            )));
            // Best-effort initial load; scanning will also attempt additive loads.
            if plugins_dir.exists() {
                let disabled = read_disabled_plugin_ids_best_effort(&plugins_dir);
                match unsafe {
                    plugins
                        .lock()
                        .expect("plugins mutex poisoned")
                        .load_dir_additive_filtered(&plugins_dir, &disabled)
                } {
                    Ok(report) => {
                        if !report.loaded.is_empty() || !report.errors.is_empty() {
                            events.emit(LibraryEvent::Log {
                                message: format!(
                                    "library plugins loaded from {}: loaded={} errors={}",
                                    plugins_dir.display(),
                                    report.loaded.len(),
                                    report.errors.len()
                                ),
                            });
                        }
                        for e in report.errors {
                            events.emit(LibraryEvent::Log {
                                message: format!("plugin load error: {e:#}"),
                            });
                        }
                    }
                    Err(e) => events.emit(LibraryEvent::Log {
                        message: format!("plugin load failed: {e:#}"),
                    }),
                }
            }

            // Ensure the in-memory plugin manager knows what is disabled (even if already loaded).
            if let Ok(mut pm) = plugins.lock() {
                pm.set_disabled_ids(read_disabled_plugin_ids_best_effort(&plugins_dir));
            }
            plugins
        };

        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        let plugins: Plugins = ();

        Ok(Self {
            pool,
            events,
            cover_dir,
            plugins_dir,
            plugins,
        })
    }
}

pub(crate) struct LibraryWorker {
    pool: SqlitePool,
    events: std::sync::Arc<EventHub>,
    cover_dir: PathBuf,
    watch_ctrl: mpsc::UnboundedSender<WatchCtrl>,
    plugins_dir: PathBuf,
    plugins: Plugins,
}

impl LibraryWorker {
    pub(crate) fn new(deps: WorkerDeps) -> Self {
        let watch_ctrl = spawn_watch_task(
            deps.pool.clone(),
            std::sync::Arc::clone(&deps.events),
            deps.cover_dir.clone(),
            deps.plugins.clone(),
        );
        Self {
            pool: deps.pool,
            events: deps.events,
            cover_dir: deps.cover_dir,
            watch_ctrl,
            plugins_dir: deps.plugins_dir,
            plugins: deps.plugins,
        }
    }

    fn refresh_plugins_best_effort(&self) {
        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
        {
            if !self.plugins_dir.exists() {
                return;
            }
            let disabled = read_disabled_plugin_ids_best_effort(&self.plugins_dir);
            let mut pm = self.plugins.lock().expect("plugins mutex poisoned");
            pm.set_disabled_ids(disabled.clone());
            let _ = unsafe { pm.load_dir_additive_filtered(&self.plugins_dir, &disabled) };
        }
    }

    pub(crate) async fn handle_command(&mut self, cmd: LibraryCommand) -> Result<()> {
        match cmd {
            LibraryCommand::AddRoot { path } => self.add_root(path).await,
            LibraryCommand::RemoveRoot { path } => self.remove_root(path).await,
            LibraryCommand::DeleteFolder { path } => self.delete_folder(path).await,
            LibraryCommand::RestoreFolder { path } => self.restore_folder(path).await,
            LibraryCommand::ListExcludedFolders => self.list_excluded_folders().await,
            LibraryCommand::ListRoots => self.list_roots().await,
            LibraryCommand::ListFolders => self.list_folders().await,
            LibraryCommand::ListTracks {
                folder,
                recursive,
                query,
                limit,
                offset,
            } => {
                self.list_tracks(folder, recursive, query, limit, offset)
                    .await
            }
            LibraryCommand::ScanAll => self.scan_all(false).await,
            LibraryCommand::ScanAllForce => self.scan_all(true).await,
            LibraryCommand::Search {
                query,
                limit,
                offset,
            } => self.search(query, limit, offset).await,
            LibraryCommand::Shutdown => Ok(()),
        }
    }

    async fn list_roots(&self) -> Result<()> {
        let roots_raw: Vec<String> = sqlx::query_scalar!(
            r#"
            SELECT path
            FROM scan_roots
            WHERE enabled=1
            ORDER BY path
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        let roots = roots_raw
            .into_iter()
            .map(|p| normalize_path_str(&p))
            .filter(|p| !p.is_empty())
            .collect::<Vec<_>>();
        self.events.emit(LibraryEvent::Roots { paths: roots });
        Ok(())
    }

    async fn list_folders(&self) -> Result<()> {
        // Distinct directories with at least one track.
        let mut dirs: Vec<String> = sqlx::query_scalar!(
            r#"
            SELECT DISTINCT dir_norm
            FROM tracks
            WHERE dir_norm <> ''
            ORDER BY dir_norm
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        // Include scan roots as top-level nodes even if they have no direct tracks.
        let roots: Vec<String> = sqlx::query_scalar!(
            r#"
            SELECT path
            FROM scan_roots
            WHERE enabled=1
            "#
        )
        .fetch_all(&self.pool)
        .await?;
        for r in roots {
            let rn = normalize_path_str(&r);
            if !rn.is_empty() {
                dirs.push(rn);
            }
        }

        // Add ancestor directories so the tree can show parent folders even if only children have tracks.
        let mut set = BTreeSet::<String>::new();
        for d in dirs {
            let mut cur = d.trim_end_matches('/').to_string();
            while !cur.is_empty() {
                set.insert(cur.clone());
                let Some(p) = parent_dir_norm(&cur) else {
                    break;
                };
                if is_drive_root(&p) {
                    break;
                }
                cur = p;
            }
        }

        self.events.emit(LibraryEvent::Folders {
            paths: set.into_iter().collect(),
        });
        Ok(())
    }

    async fn list_excluded_folders(&self) -> Result<()> {
        let rows: Vec<String> = sqlx::query_scalar(
            r#"
            SELECT path
            FROM excluded_folders
            ORDER BY path
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let paths = rows
            .into_iter()
            .map(|p| normalize_path_str(&p))
            .filter(|p| !p.is_empty())
            .collect::<Vec<_>>();

        self.events.emit(LibraryEvent::ExcludedFolders { paths });
        Ok(())
    }

    async fn list_tracks(
        &self,
        folder: String,
        recursive: bool,
        query: String,
        limit: i64,
        offset: i64,
    ) -> Result<()> {
        let folder = normalize_path_str(&folder);
        let query = query.trim().to_string();
        let limit = limit.max(1).min(5000);
        let offset = offset.max(0);

        let rows = if query.is_empty() {
            if folder.is_empty() {
                sqlx::query_as!(
                    tracks::TrackLiteRow,
                    r#"
                    SELECT id as "id!", path, title, artist, album, duration_ms
                    FROM tracks
                    ORDER BY id DESC
                    LIMIT ?1 OFFSET ?2
                    "#,
                    limit,
                    offset
                )
                .fetch_all(&self.pool)
                .await
                .context("list tracks failed")?
            } else if recursive {
                let like = format!("{folder}/%");
                sqlx::query_as!(
                    tracks::TrackLiteRow,
                    r#"
                    SELECT id as "id!", path, title, artist, album, duration_ms
                    FROM tracks
                    WHERE path_norm LIKE ?1
                    ORDER BY id DESC
                    LIMIT ?2 OFFSET ?3
                    "#,
                    like,
                    limit,
                    offset
                )
                .fetch_all(&self.pool)
                .await
                .context("list tracks (recursive) failed")?
            } else {
                sqlx::query_as!(
                    tracks::TrackLiteRow,
                    r#"
                    SELECT id as "id!", path, title, artist, album, duration_ms
                    FROM tracks
                    WHERE dir_norm = ?1
                    ORDER BY id DESC
                    LIMIT ?2 OFFSET ?3
                    "#,
                    folder,
                    limit,
                    offset
                )
                .fetch_all(&self.pool)
                .await
                .context("list tracks (folder) failed")?
            }
        } else {
            let fts = build_fts_query(&query);
            if folder.is_empty() {
                sqlx::query_as!(
                    tracks::TrackLiteRow,
                    r#"
                    SELECT t.id as "id!", t.path, t.title, t.artist, t.album, t.duration_ms
                    FROM tracks_fts
                    JOIN tracks t ON t.id = tracks_fts.rowid
                    WHERE tracks_fts MATCH ?1
                    ORDER BY bm25(tracks_fts)
                    LIMIT ?2 OFFSET ?3
                    "#,
                    fts,
                    limit,
                    offset
                )
                .fetch_all(&self.pool)
                .await
                .with_context(|| format!("fts query failed: {fts}"))?
            } else if recursive {
                let like = format!("{folder}/%");
                sqlx::query_as!(
                    tracks::TrackLiteRow,
                    r#"
                    SELECT t.id as "id!", t.path, t.title, t.artist, t.album, t.duration_ms
                    FROM tracks_fts
                    JOIN tracks t ON t.id = tracks_fts.rowid
                    WHERE tracks_fts MATCH ?1 AND t.path_norm LIKE ?2
                    ORDER BY bm25(tracks_fts)
                    LIMIT ?3 OFFSET ?4
                    "#,
                    fts,
                    like,
                    limit,
                    offset
                )
                .fetch_all(&self.pool)
                .await
                .with_context(|| format!("fts query failed: {fts}"))?
            } else {
                sqlx::query_as!(
                    tracks::TrackLiteRow,
                    r#"
                    SELECT t.id as "id!", t.path, t.title, t.artist, t.album, t.duration_ms
                    FROM tracks_fts
                    JOIN tracks t ON t.id = tracks_fts.rowid
                    WHERE tracks_fts MATCH ?1 AND t.dir_norm = ?2
                    ORDER BY bm25(tracks_fts)
                    LIMIT ?3 OFFSET ?4
                    "#,
                    fts,
                    folder,
                    limit,
                    offset
                )
                .fetch_all(&self.pool)
                .await
                .with_context(|| format!("fts query failed: {fts}"))?
            }
        };

        let items = rows
            .into_iter()
            .map(|row| TrackLite {
                id: row.id,
                path: row.path,
                title: row.title,
                artist: row.artist,
                album: row.album,
                duration_ms: row.duration_ms,
            })
            .collect::<Vec<_>>();

        self.events.emit(LibraryEvent::Tracks {
            folder,
            recursive,
            query,
            items,
        });
        Ok(())
    }

    async fn add_root(&self, path: String) -> Result<()> {
        let path = normalize_path_str(&path);
        sqlx::query!(
            r#"
            INSERT INTO scan_roots(path, enabled, last_scan_ms)
            VALUES(?1, 1, 0)
            ON CONFLICT(path) DO UPDATE SET enabled=1
            "#,
            path
        )
        .execute(&self.pool)
        .await?;

        self.events.emit(LibraryEvent::Log {
            message: format!("added scan root: {}", path),
        });
        self.list_roots().await?;
        let _ = self.watch_ctrl.send(WatchCtrl::Refresh);
        Ok(())
    }

    async fn remove_root(&self, path: String) -> Result<()> {
        let path = normalize_path_str(&path);
        sqlx::query(
            "UPDATE scan_roots SET enabled=0 WHERE rtrim(replace(path,'\\\\','/'),'/') = ?1",
        )
        .bind(&path)
        .execute(&self.pool)
        .await?;
        self.events.emit(LibraryEvent::Log {
            message: format!("disabled scan root: {}", path),
        });
        self.list_roots().await?;
        let _ = self.watch_ctrl.send(WatchCtrl::Refresh);
        Ok(())
    }

    async fn delete_folder(&self, path: String) -> Result<()> {
        let folder = normalize_path_str(&path);
        if folder.is_empty() || is_drive_root(&folder) {
            return Ok(());
        }

        // Persist exclusion so future scans won't bring it back.
        sqlx::query(
            r#"
            INSERT INTO excluded_folders(path)
            VALUES(?1)
            ON CONFLICT(path) DO NOTHING
            "#,
        )
        .bind(&folder)
        .execute(&self.pool)
        .await?;

        // Remove matching scan roots entirely.
        sqlx::query("DELETE FROM scan_roots WHERE rtrim(replace(path,'\\\\','/'),'/') = ?1")
            .bind(&folder)
            .execute(&self.pool)
            .await?;

        let like = format!("{folder}/%");
        let ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM tracks WHERE path_norm LIKE ?1")
            .bind(&like)
            .fetch_all(&self.pool)
            .await?;

        let deleted = sqlx::query("DELETE FROM tracks WHERE path_norm LIKE ?1")
            .bind(&like)
            .execute(&self.pool)
            .await?
            .rows_affected();

        // Best-effort cover cleanup.
        for id in ids {
            let final_path = self.cover_dir.join(id.to_string());
            let tmp_path = self.cover_dir.join(format!("{id}.tmp"));
            let _ = std::fs::remove_file(final_path);
            let _ = std::fs::remove_file(tmp_path);
        }

        self.events.emit(LibraryEvent::Log {
            message: format!("deleted folder: {folder} ({deleted} tracks)"),
        });
        self.list_roots().await?;
        self.list_folders().await?;
        self.list_excluded_folders().await?;
        let _ = self.watch_ctrl.send(WatchCtrl::Refresh);
        self.events.emit(LibraryEvent::Changed);
        Ok(())
    }

    async fn restore_folder(&self, path: String) -> Result<()> {
        let folder = normalize_path_str(&path);
        if folder.is_empty() || is_drive_root(&folder) {
            return Ok(());
        }

        sqlx::query("DELETE FROM excluded_folders WHERE path=?1")
            .bind(&folder)
            .execute(&self.pool)
            .await?;

        self.events.emit(LibraryEvent::Log {
            message: format!("restored folder: {folder}"),
        });
        self.list_excluded_folders().await?;
        let _ = self.watch_ctrl.send(WatchCtrl::Refresh);

        // Re-import existing files immediately (async) so users don't have to
        // wait for the next filesystem change event.
        let pool = self.pool.clone();
        let events = std::sync::Arc::clone(&self.events);
        let cover_dir = self.cover_dir.clone();
        let plugins = self.plugins.clone();
        tokio::spawn(async move {
            match scan::scan_folder_into_db(pool, &events, &cover_dir, &plugins, &folder).await {
                Ok(true) => events.emit(LibraryEvent::Changed),
                Ok(false) => {}
                Err(e) => events.emit(LibraryEvent::Log {
                    message: format!("restore scan failed: {e:#}"),
                }),
            }
        });

        self.events.emit(LibraryEvent::Changed);
        Ok(())
    }

    async fn scan_all(&self, force: bool) -> Result<()> {
        self.refresh_plugins_best_effort();
        scan::scan_all(
            &self.pool,
            &self.events,
            &self.cover_dir,
            &self.plugins,
            force,
        )
        .await
    }

    async fn search(&self, query: String, limit: i64, offset: i64) -> Result<()> {
        let query = query.trim().to_string();
        let limit = limit.max(1).min(200);
        let offset = offset.max(0);

        let rows = if query.is_empty() {
            sqlx::query_as!(
                tracks::TrackLiteRow,
                r#"
            SELECT id as "id!", path, title, artist, album, duration_ms
            FROM tracks
            ORDER BY id DESC
            LIMIT ?1 OFFSET ?2
            "#,
                limit,
                offset
            )
            .fetch_all(&self.pool)
            .await
            .context("list tracks failed")?
        } else {
            let fts = build_fts_query(&query);
            sqlx::query_as!(
                tracks::TrackLiteRow,
                r#"
            SELECT t.id as "id!", t.path, t.title, t.artist, t.album, t.duration_ms
            FROM tracks_fts
            JOIN tracks t ON t.id = tracks_fts.rowid
            WHERE tracks_fts MATCH ?1
            ORDER BY bm25(tracks_fts)
            LIMIT ?2 OFFSET ?3
            "#,
                fts,
                limit,
                offset
            )
            .fetch_all(&self.pool)
            .await
            .with_context(|| format!("fts query failed: {fts}"))?
        };

        let items = rows
            .into_iter()
            .map(|row| TrackLite {
                id: row.id,
                path: row.path,
                title: row.title,
                artist: row.artist,
                album: row.album,
                duration_ms: row.duration_ms,
            })
            .collect::<Vec<_>>();

        self.events
            .emit(LibraryEvent::SearchResult { query, items });
        Ok(())
    }
}
