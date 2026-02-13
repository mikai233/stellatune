pub(crate) mod db;
mod fts;
mod metadata;
mod paths;
mod scan;
mod tracks;
mod watch;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use sqlx::{FromRow, QueryBuilder, SqlitePool};
use tokio::sync::mpsc;
use tokio::time::timeout;

use stellatune_core::{LibraryCommand, LibraryEvent, PlaylistLite, TrackLite};

use crate::service::EventHub;

use self::fts::build_fts_query;
use self::metadata::clear_metadata_decoder_cache;
use self::paths::{is_drive_root, normalize_path_str, parent_dir_norm};
use self::watch::{WatchCtrl, spawn_watch_task};

#[derive(Debug, FromRow)]
struct PlaylistLiteRow {
    id: i64,
    name: String,
    system_key: Option<String>,
    track_count: i64,
    first_track_id: Option<i64>,
}

pub(crate) struct WorkerDeps {
    pool: SqlitePool,
    events: std::sync::Arc<EventHub>,
    cover_dir: PathBuf,
    plugins_dir: PathBuf,
}

impl WorkerDeps {
    pub(crate) async fn new(
        db_path: &Path,
        events: std::sync::Arc<EventHub>,
        plugins_dir: PathBuf,
    ) -> Result<Self> {
        let pool = db::init_db(db_path).await?;

        let cover_dir = db_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("covers");
        std::fs::create_dir_all(&cover_dir).with_context(|| {
            format!("failed to create cover cache dir: {}", cover_dir.display())
        })?;

        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
        {
            if plugins_dir.exists() {
                tracing::info!(
                    plugins_dir = %plugins_dir.display(),
                    "library plugin bootstrap begin"
                );
                clear_metadata_decoder_cache();
                let disabled = db::list_disabled_plugin_ids(&pool)
                    .await
                    .unwrap_or_default();
                let service = stellatune_plugins::runtime::handle::shared_runtime_service();
                service.set_disabled_plugin_ids_async(disabled).await;
                match timeout(
                    Duration::from_secs(8),
                    service.reload_dir_from_state_async(&plugins_dir),
                )
                .await
                {
                    Ok(Ok(v2)) => {
                        tracing::info!(
                            loaded = v2.loaded.len(),
                            deactivated = v2.deactivated.len(),
                            errors = v2.errors.len(),
                            reclaimed_leases = v2.reclaimed_leases,
                            "library plugin bootstrap reload completed"
                        );
                        events.emit(LibraryEvent::Log {
                            message: format!(
                                "library plugin runtime v2 reload: loaded={} deactivated={} errors={} reclaimed_leases={}",
                                v2.loaded.len(),
                                v2.deactivated.len(),
                                v2.errors.len(),
                                v2.reclaimed_leases
                            ),
                        });
                    }
                    Ok(Err(e)) => {
                        tracing::warn!(error = %format!("{e:#}"), "library plugin bootstrap reload failed");
                        events.emit(LibraryEvent::Log {
                            message: format!("library plugin runtime v2 reload failed: {e:#}"),
                        });
                    }
                    Err(_) => {
                        tracing::warn!("library plugin bootstrap reload timed out (8s)");
                        events.emit(LibraryEvent::Log {
                            message: "library plugin runtime v2 reload timed out (8s)".to_string(),
                        });
                    }
                }
                tracing::info!("library plugin bootstrap end");
            }
        }

        Ok(Self {
            pool,
            events,
            cover_dir,
            plugins_dir,
        })
    }
}

pub(crate) struct LibraryWorker {
    pool: SqlitePool,
    events: std::sync::Arc<EventHub>,
    cover_dir: PathBuf,
    watch_ctrl: mpsc::UnboundedSender<WatchCtrl>,
    plugins_dir: PathBuf,
}

impl LibraryWorker {
    pub(crate) fn new(deps: WorkerDeps) -> Self {
        let watch_ctrl = spawn_watch_task(
            deps.pool.clone(),
            std::sync::Arc::clone(&deps.events),
            deps.cover_dir.clone(),
        );
        Self {
            pool: deps.pool,
            events: deps.events,
            cover_dir: deps.cover_dir,
            watch_ctrl,
            plugins_dir: deps.plugins_dir,
        }
    }

    async fn refresh_plugins_best_effort(&self) {
        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
        {
            if !self.plugins_dir.exists() {
                return;
            }
            clear_metadata_decoder_cache();
            let disabled = db::list_disabled_plugin_ids(&self.pool)
                .await
                .unwrap_or_default();
            let service = stellatune_plugins::runtime::handle::shared_runtime_service();
            service.set_disabled_plugin_ids_async(disabled).await;
            let _ = timeout(
                Duration::from_secs(8),
                service.reload_dir_from_state_async(&self.plugins_dir),
            )
            .await;
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
            LibraryCommand::ListPlaylists => self.list_playlists().await,
            LibraryCommand::CreatePlaylist { name } => self.create_playlist(name).await,
            LibraryCommand::RenamePlaylist { id, name } => self.rename_playlist(id, name).await,
            LibraryCommand::DeletePlaylist { id } => self.delete_playlist(id).await,
            LibraryCommand::ListPlaylistTracks {
                playlist_id,
                query,
                limit,
                offset,
            } => {
                self.list_playlist_tracks(playlist_id, query, limit, offset)
                    .await
            }
            LibraryCommand::AddTrackToPlaylist {
                playlist_id,
                track_id,
            } => self.add_track_to_playlist(playlist_id, track_id).await,
            LibraryCommand::AddTracksToPlaylist {
                playlist_id,
                track_ids,
            } => self.add_tracks_to_playlist(playlist_id, track_ids).await,
            LibraryCommand::RemoveTrackFromPlaylist {
                playlist_id,
                track_id,
            } => self.remove_track_from_playlist(playlist_id, track_id).await,
            LibraryCommand::RemoveTracksFromPlaylist {
                playlist_id,
                track_ids,
            } => {
                self.remove_tracks_from_playlist(playlist_id, track_ids)
                    .await
            }
            LibraryCommand::MoveTrackInPlaylist {
                playlist_id,
                track_id,
                new_index,
            } => {
                self.move_track_in_playlist(playlist_id, track_id, new_index)
                    .await
            }
            LibraryCommand::ListLikedTrackIds => self.list_liked_track_ids().await,
            LibraryCommand::SetTrackLiked { track_id, liked } => {
                self.set_track_liked(track_id, liked).await
            }
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
        let limit = limit.clamp(1, 5000);
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

    async fn list_playlists(&self) -> Result<()> {
        let rows = sqlx::query_as::<_, PlaylistLiteRow>(
            r#"
            SELECT
              p.id,
              p.name,
              p.system_key,
              CAST(COUNT(pt.track_id) AS INTEGER) AS track_count,
              (
                SELECT pt2.track_id
                FROM playlist_tracks pt2
                WHERE pt2.playlist_id = p.id
                ORDER BY pt2.sort_index ASC, pt2.track_id ASC
                LIMIT 1
              ) AS first_track_id
            FROM playlists p
            LEFT JOIN playlist_tracks pt ON pt.playlist_id = p.id
            GROUP BY p.id
            ORDER BY
              CASE WHEN p.system_key = 'liked' THEN 0 ELSE 1 END,
              p.name COLLATE NOCASE,
              p.id
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let items = rows
            .into_iter()
            .map(|row| PlaylistLite {
                id: row.id,
                name: row.name,
                system_key: row.system_key,
                track_count: row.track_count,
                first_track_id: row.first_track_id,
            })
            .collect::<Vec<_>>();

        self.events.emit(LibraryEvent::Playlists { items });
        Ok(())
    }

    async fn create_playlist(&self, name: String) -> Result<()> {
        let name = name.trim();
        if name.is_empty() {
            return Ok(());
        }

        sqlx::query(
            r#"
            INSERT INTO playlists(name, system_key)
            VALUES(?1, NULL)
            "#,
        )
        .bind(name)
        .execute(&self.pool)
        .await?;

        self.events.emit(LibraryEvent::Changed);
        Ok(())
    }

    async fn rename_playlist(&self, id: i64, name: String) -> Result<()> {
        let name = name.trim();
        if id <= 0 || name.is_empty() {
            return Ok(());
        }

        sqlx::query(
            r#"
            UPDATE playlists
            SET name = ?1
            WHERE id = ?2 AND system_key IS NULL
            "#,
        )
        .bind(name)
        .bind(id)
        .execute(&self.pool)
        .await?;

        self.events.emit(LibraryEvent::Changed);
        Ok(())
    }

    async fn delete_playlist(&self, id: i64) -> Result<()> {
        if id <= 0 {
            return Ok(());
        }

        sqlx::query(
            r#"
            DELETE FROM playlists
            WHERE id = ?1 AND system_key IS NULL
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await?;

        self.events.emit(LibraryEvent::Changed);
        Ok(())
    }

    async fn list_playlist_tracks(
        &self,
        playlist_id: i64,
        query: String,
        limit: i64,
        offset: i64,
    ) -> Result<()> {
        if playlist_id <= 0 {
            self.events.emit(LibraryEvent::PlaylistTracks {
                playlist_id,
                query: query.trim().to_string(),
                items: Vec::new(),
            });
            return Ok(());
        }

        let query = query.trim().to_string();
        let limit = limit.clamp(1, 5000);
        let offset = offset.max(0);

        let rows = if query.is_empty() {
            sqlx::query_as::<_, tracks::TrackLiteRow>(
                r#"
                SELECT t.id, t.path, t.title, t.artist, t.album, t.duration_ms
                FROM playlist_tracks pt
                JOIN tracks t ON t.id = pt.track_id
                WHERE pt.playlist_id = ?1
                ORDER BY pt.sort_index ASC, pt.track_id ASC
                LIMIT ?2 OFFSET ?3
                "#,
            )
            .bind(playlist_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await
            .context("list playlist tracks failed")?
        } else {
            let fts = build_fts_query(&query);
            sqlx::query_as::<_, tracks::TrackLiteRow>(
                r#"
                SELECT t.id, t.path, t.title, t.artist, t.album, t.duration_ms
                FROM tracks_fts
                JOIN tracks t ON t.id = tracks_fts.rowid
                JOIN playlist_tracks pt ON pt.track_id = t.id
                WHERE pt.playlist_id = ?1 AND tracks_fts MATCH ?2
                ORDER BY pt.sort_index ASC, pt.track_id ASC
                LIMIT ?3 OFFSET ?4
                "#,
            )
            .bind(playlist_id)
            .bind(fts)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await
            .context("list playlist tracks with fts failed")?
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

        self.events.emit(LibraryEvent::PlaylistTracks {
            playlist_id,
            query,
            items,
        });
        Ok(())
    }

    async fn add_track_to_playlist(&self, playlist_id: i64, track_id: i64) -> Result<()> {
        if playlist_id <= 0 || track_id <= 0 {
            return Ok(());
        }
        sqlx::query(
            r#"
            INSERT INTO playlist_tracks(playlist_id, track_id, sort_index)
            SELECT
              ?1,
              ?2,
              COALESCE((
                SELECT MAX(pt.sort_index) + 1
                FROM playlist_tracks pt
                WHERE pt.playlist_id = ?1
              ), 0)
            ON CONFLICT(playlist_id, track_id) DO NOTHING
            "#,
        )
        .bind(playlist_id)
        .bind(track_id)
        .execute(&self.pool)
        .await?;

        self.events.emit(LibraryEvent::Changed);
        Ok(())
    }

    async fn remove_track_from_playlist(&self, playlist_id: i64, track_id: i64) -> Result<()> {
        if playlist_id <= 0 || track_id <= 0 {
            return Ok(());
        }
        sqlx::query(
            r#"
            DELETE FROM playlist_tracks
            WHERE playlist_id = ?1 AND track_id = ?2
            "#,
        )
        .bind(playlist_id)
        .bind(track_id)
        .execute(&self.pool)
        .await?;

        self.normalize_playlist_order(playlist_id).await?;
        self.events.emit(LibraryEvent::Changed);
        Ok(())
    }

    async fn add_tracks_to_playlist(&self, playlist_id: i64, track_ids: Vec<i64>) -> Result<()> {
        if playlist_id <= 0 {
            return Ok(());
        }
        let mut ids = track_ids
            .into_iter()
            .filter(|id| *id > 0)
            .collect::<Vec<_>>();
        ids.sort_unstable();
        ids.dedup();
        if ids.is_empty() {
            return Ok(());
        }

        let mut tx = self.pool.begin().await?;
        let mut sort_index = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COALESCE(MAX(sort_index) + 1, 0)
            FROM playlist_tracks
            WHERE playlist_id = ?1
            "#,
        )
        .bind(playlist_id)
        .fetch_one(&mut *tx)
        .await?;

        for track_id in ids {
            let result = sqlx::query(
                r#"
                INSERT INTO playlist_tracks(playlist_id, track_id, sort_index)
                VALUES(?1, ?2, ?3)
                ON CONFLICT(playlist_id, track_id) DO NOTHING
                "#,
            )
            .bind(playlist_id)
            .bind(track_id)
            .bind(sort_index)
            .execute(&mut *tx)
            .await?;
            if result.rows_affected() > 0 {
                sort_index += 1;
            }
        }

        tx.commit().await?;
        self.events.emit(LibraryEvent::Changed);
        Ok(())
    }

    async fn remove_tracks_from_playlist(
        &self,
        playlist_id: i64,
        track_ids: Vec<i64>,
    ) -> Result<()> {
        if playlist_id <= 0 {
            return Ok(());
        }
        let mut ids = track_ids
            .into_iter()
            .filter(|id| *id > 0)
            .collect::<Vec<_>>();
        ids.sort_unstable();
        ids.dedup();
        if ids.is_empty() {
            return Ok(());
        }

        let mut qb = QueryBuilder::new("DELETE FROM playlist_tracks WHERE playlist_id = ");
        qb.push_bind(playlist_id);
        qb.push(" AND track_id IN (");
        {
            let mut separated = qb.separated(", ");
            for id in ids {
                separated.push_bind(id);
            }
        }
        qb.push(")");
        qb.build().execute(&self.pool).await?;

        self.normalize_playlist_order(playlist_id).await?;
        self.events.emit(LibraryEvent::Changed);
        Ok(())
    }

    async fn move_track_in_playlist(
        &self,
        playlist_id: i64,
        track_id: i64,
        new_index: i64,
    ) -> Result<()> {
        if playlist_id <= 0 || track_id <= 0 {
            return Ok(());
        }

        let mut order = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT track_id
            FROM playlist_tracks
            WHERE playlist_id = ?1
            ORDER BY sort_index ASC, track_id ASC
            "#,
        )
        .bind(playlist_id)
        .fetch_all(&self.pool)
        .await?;

        let Some(old_index) = order.iter().position(|id| *id == track_id) else {
            return Ok(());
        };
        let item = order.remove(old_index);

        let mut dst = new_index.max(0) as usize;
        if dst > order.len() {
            dst = order.len();
        }
        order.insert(dst, item);

        let mut tx = self.pool.begin().await?;
        for (idx, id) in order.into_iter().enumerate() {
            sqlx::query(
                r#"
                UPDATE playlist_tracks
                SET sort_index = ?1
                WHERE playlist_id = ?2 AND track_id = ?3
                "#,
            )
            .bind(idx as i64)
            .bind(playlist_id)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;

        self.events.emit(LibraryEvent::Changed);
        Ok(())
    }

    async fn list_liked_track_ids(&self) -> Result<()> {
        let liked_id = self.liked_playlist_id().await?;
        let track_ids = if let Some(playlist_id) = liked_id {
            sqlx::query_scalar::<_, i64>(
                r#"
                SELECT track_id
                FROM playlist_tracks
                WHERE playlist_id = ?1
                "#,
            )
            .bind(playlist_id)
            .fetch_all(&self.pool)
            .await?
        } else {
            Vec::new()
        };
        self.events.emit(LibraryEvent::LikedTrackIds { track_ids });
        Ok(())
    }

    async fn set_track_liked(&self, track_id: i64, liked: bool) -> Result<()> {
        if track_id <= 0 {
            return Ok(());
        }
        let Some(playlist_id) = self.liked_playlist_id().await? else {
            return Ok(());
        };

        if liked {
            sqlx::query(
                r#"
                INSERT INTO playlist_tracks(playlist_id, track_id, sort_index)
                SELECT
                  ?1,
                  ?2,
                  COALESCE((
                    SELECT MAX(pt.sort_index) + 1
                    FROM playlist_tracks pt
                    WHERE pt.playlist_id = ?1
                  ), 0)
                ON CONFLICT(playlist_id, track_id) DO NOTHING
                "#,
            )
            .bind(playlist_id)
            .bind(track_id)
            .execute(&self.pool)
            .await?;
        } else {
            sqlx::query(
                r#"
                DELETE FROM playlist_tracks
                WHERE playlist_id = ?1 AND track_id = ?2
                "#,
            )
            .bind(playlist_id)
            .bind(track_id)
            .execute(&self.pool)
            .await?;
            self.normalize_playlist_order(playlist_id).await?;
        }

        self.events.emit(LibraryEvent::Changed);
        Ok(())
    }

    async fn liked_playlist_id(&self) -> Result<Option<i64>> {
        sqlx::query_scalar::<_, i64>(
            r#"
            SELECT id
            FROM playlists
            WHERE system_key = 'liked'
            LIMIT 1
            "#,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(Into::into)
    }

    async fn normalize_playlist_order(&self, playlist_id: i64) -> Result<()> {
        let track_ids = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT track_id
            FROM playlist_tracks
            WHERE playlist_id = ?1
            ORDER BY sort_index ASC, track_id ASC
            "#,
        )
        .bind(playlist_id)
        .fetch_all(&self.pool)
        .await?;

        let mut tx = self.pool.begin().await?;
        for (idx, track_id) in track_ids.into_iter().enumerate() {
            sqlx::query(
                r#"
                UPDATE playlist_tracks
                SET sort_index = ?1
                WHERE playlist_id = ?2 AND track_id = ?3
                "#,
            )
            .bind(idx as i64)
            .bind(playlist_id)
            .bind(track_id)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
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
        tokio::spawn(async move {
            match scan::scan_folder_into_db(pool, &events, &cover_dir, &folder).await {
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
        self.refresh_plugins_best_effort().await;
        scan::scan_all(&self.pool, &self.events, &self.cover_dir, force).await
    }

    async fn search(&self, query: String, limit: i64, offset: i64) -> Result<()> {
        let query = query.trim().to_string();
        let limit = limit.clamp(1, 200);
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
