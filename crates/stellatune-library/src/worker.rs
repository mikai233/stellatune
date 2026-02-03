use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use tracing::debug;
use walkdir::WalkDir;

use stellatune_core::{LibraryCommand, LibraryEvent, TrackLite};

use crate::service::EventHub;

pub(crate) struct WorkerDeps {
    pool: SqlitePool,
    events: std::sync::Arc<EventHub>,
}

impl WorkerDeps {
    pub(crate) async fn new(db_path: &Path, events: std::sync::Arc<EventHub>) -> Result<Self> {
        let pool = init_db(db_path).await?;
        Ok(Self { pool, events })
    }
}

pub(crate) struct LibraryWorker {
    pool: SqlitePool,
    events: std::sync::Arc<EventHub>,
}

impl LibraryWorker {
    pub(crate) fn new(deps: WorkerDeps) -> Self {
        Self {
            pool: deps.pool,
            events: deps.events,
        }
    }

    pub(crate) async fn handle_command(&mut self, cmd: LibraryCommand) -> Result<()> {
        match cmd {
            LibraryCommand::AddRoot { path } => self.add_root(path).await,
            LibraryCommand::RemoveRoot { path } => self.remove_root(path).await,
            LibraryCommand::ScanAll => self.scan_all().await,
            LibraryCommand::Search {
                query,
                limit,
                offset,
            } => self.search(query, limit, offset).await,
            LibraryCommand::Shutdown => Ok(()),
        }
    }

    async fn add_root(&self, path: String) -> Result<()> {
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
        Ok(())
    }

    async fn remove_root(&self, path: String) -> Result<()> {
        sqlx::query!("UPDATE scan_roots SET enabled=0 WHERE path=?1", path)
            .execute(&self.pool)
            .await?;
        self.events.emit(LibraryEvent::Log {
            message: format!("disabled scan root: {}", path),
        });
        Ok(())
    }

    async fn scan_all(&self) -> Result<()> {
        let roots: Vec<String> = sqlx::query_scalar!("SELECT path FROM scan_roots WHERE enabled=1")
            .fetch_all(&self.pool)
            .await?;

        let started = Instant::now();
        let scan_started_ms = now_ms();
        let mut scanned: u64 = 0;
        let mut upserted: u64 = 0;
        let mut skipped: u64 = 0;
        let mut errors: u64 = 0;

        for root in roots {
            self.events.emit(LibraryEvent::Log {
                message: format!("scanning: {root}"),
            });

            let (tx, mut rx) = tokio::sync::mpsc::channel::<FileCandidate>(512);
            let root_clone = root.clone();

            // Blocking filesystem enumeration & metadata.
            let walker = tokio::task::spawn_blocking(move || {
                for entry in WalkDir::new(&root_clone).follow_links(false).into_iter() {
                    let entry = match entry {
                        Ok(e) => e,
                        Err(_) => continue,
                    };
                    if !entry.file_type().is_file() {
                        continue;
                    }
                    let path = entry.path().to_path_buf();
                    let ext = path
                        .extension()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_ascii_lowercase();
                    if !is_audio_ext(&ext) {
                        continue;
                    }
                    let meta = match entry.metadata() {
                        Ok(m) => m,
                        Err(_) => continue,
                    };
                    let mtime_ms = meta
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_millis() as i64)
                        .unwrap_or(0);
                    let size_bytes = meta.len() as i64;
                    let path_str = path.to_string_lossy().to_string();
                    if tx
                        .blocking_send(FileCandidate {
                            path: path_str,
                            ext,
                            mtime_ms,
                            size_bytes,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            });

            while let Some(file) = rx.recv().await {
                scanned += 1;

                // Skip unchanged.
                if let Some((old_mtime, old_size)) =
                    select_track_mtime_size(&self.pool, &file.path).await?
                {
                    if old_mtime == file.mtime_ms && old_size == file.size_bytes {
                        skipped += 1;
                        continue;
                    }
                }

                if let Err(e) = upsert_track(
                    &self.pool,
                    &file.path,
                    &file.ext,
                    file.mtime_ms,
                    file.size_bytes,
                )
                .await
                {
                    errors += 1;
                    self.events.emit(LibraryEvent::Log {
                        message: format!("upsert error: {}: {e}", file.path),
                    });
                    continue;
                }

                upserted += 1;

                if scanned % 500 == 0 {
                    self.events.emit(LibraryEvent::ScanProgress {
                        scanned: scanned as i64,
                        updated: upserted as i64,
                        skipped: skipped as i64,
                        errors: errors as i64,
                    });
                }
            }

            if let Err(join_err) = walker.await {
                errors += 1;
                self.events.emit(LibraryEvent::Log {
                    message: format!("walk task failed: {join_err}"),
                });
            }

            sqlx::query!(
                "UPDATE scan_roots SET last_scan_ms=?1 WHERE path=?2",
                scan_started_ms,
                root
            )
            .execute(&self.pool)
            .await?;
        }

        self.events.emit(LibraryEvent::ScanProgress {
            scanned: scanned as i64,
            updated: upserted as i64,
            skipped: skipped as i64,
            errors: errors as i64,
        });
        self.events.emit(LibraryEvent::ScanFinished {
            duration_ms: started.elapsed().as_millis() as i64,
            scanned: scanned as i64,
            updated: upserted as i64,
            skipped: skipped as i64,
            errors: errors as i64,
        });

        Ok(())
    }

    async fn search(&self, query: String, limit: i64, offset: i64) -> Result<()> {
        let query = query.trim().to_string();
        if query.is_empty() {
            self.events.emit(LibraryEvent::SearchResult {
                query,
                items: Vec::new(),
            });
            return Ok(());
        }

        let fts = build_fts_query(&query);
        let limit = limit.max(1).min(200);
        let offset = offset.max(0);
        let rows = sqlx::query_as!(
            TrackLiteRow,
            r#"
            SELECT t.id, t.path, t.title, t.artist, t.album, t.duration_ms
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
        .with_context(|| format!("fts query failed: {fts}"))?;

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

#[derive(Debug)]
struct TrackLiteRow {
    id: i64,
    path: String,
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    duration_ms: Option<i64>,
}

fn is_audio_ext(ext: &str) -> bool {
    matches!(ext, "mp3" | "flac" | "wav")
}

fn build_fts_query(q: &str) -> String {
    // Simple prefix query:
    //   "foo bar" => "foo* AND bar*"
    q.split_whitespace()
        .filter(|s| !s.is_empty())
        .map(|s| format!("{s}*"))
        .collect::<Vec<_>>()
        .join(" AND ")
}

async fn init_db(db_path: &Path) -> Result<SqlitePool> {
    let mut opts = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(std::time::Duration::from_secs(3));

    // Helpful runtime pragmas.
    opts = opts.pragma("temp_store", "MEMORY");
    opts = opts.pragma("foreign_keys", "ON");

    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(opts)
        .await
        .context("failed to connect sqlite")?;

    ensure_fts5(&pool).await?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .context("failed to run migrations")?;

    debug!("sqlite ready: {}", db_path.display());
    Ok(pool)
}

async fn ensure_fts5(pool: &SqlitePool) -> Result<()> {
    // Best-effort: SQLite might not be compiled with compileoption reporting, so we also rely on
    // the migration creation to fail if FTS5 is missing. This check exists for a clearer error.
    let enabled: Option<i64> =
        sqlx::query_scalar("SELECT sqlite_compileoption_used('ENABLE_FTS5')")
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();
    if let Some(0) = enabled {
        anyhow::bail!("SQLite is missing FTS5 support (ENABLE_FTS5=0)");
    }
    Ok(())
}

async fn select_track_mtime_size(pool: &SqlitePool, path: &str) -> Result<Option<(i64, i64)>> {
    let row = sqlx::query!(
        "SELECT mtime_ms, size_bytes FROM tracks WHERE path=?1",
        path
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| (r.mtime_ms, r.size_bytes)))
}

async fn upsert_track(
    pool: &SqlitePool,
    path: &str,
    ext: &str,
    mtime_ms: i64,
    size_bytes: i64,
) -> Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO tracks(path, ext, mtime_ms, size_bytes)
        VALUES(?1, ?2, ?3, ?4)
        ON CONFLICT(path) DO UPDATE SET
            ext=excluded.ext,
            mtime_ms=excluded.mtime_ms,
            size_bytes=excluded.size_bytes
        "#,
        path,
        ext,
        mtime_ms,
        size_bytes
    )
    .execute(pool)
    .await?;
    Ok(())
}

struct FileCandidate {
    path: String,
    ext: String,
    mtime_ms: i64,
    size_bytes: i64,
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
