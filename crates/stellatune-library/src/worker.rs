use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use tracing::debug;
use walkdir::WalkDir;

use stellatune_core::{LibraryCommand, LibraryEvent, TrackLite};

use crate::service::EventHub;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::{Limit, MetadataOptions, StandardTagKey, StandardVisualKey, Value};
use symphonia::core::probe::Hint;
use symphonia::default::get_probe;

pub(crate) struct WorkerDeps {
    pool: SqlitePool,
    events: std::sync::Arc<EventHub>,
    cover_dir: PathBuf,
}

impl WorkerDeps {
    pub(crate) async fn new(db_path: &Path, events: std::sync::Arc<EventHub>) -> Result<Self> {
        let pool = init_db(db_path).await?;

        let cover_dir = db_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("covers");
        std::fs::create_dir_all(&cover_dir).with_context(|| {
            format!("failed to create cover cache dir: {}", cover_dir.display())
        })?;

        Ok(Self {
            pool,
            events,
            cover_dir,
        })
    }
}

pub(crate) struct LibraryWorker {
    pool: SqlitePool,
    events: std::sync::Arc<EventHub>,
    cover_dir: PathBuf,
}

impl LibraryWorker {
    pub(crate) fn new(deps: WorkerDeps) -> Self {
        Self {
            pool: deps.pool,
            events: deps.events,
            cover_dir: deps.cover_dir,
        }
    }

    pub(crate) async fn handle_command(&mut self, cmd: LibraryCommand) -> Result<()> {
        match cmd {
            LibraryCommand::AddRoot { path } => self.add_root(path).await,
            LibraryCommand::RemoveRoot { path } => self.remove_root(path).await,
            LibraryCommand::ListRoots => self.list_roots().await,
            LibraryCommand::ScanAll => self.scan_all().await,
            LibraryCommand::Search {
                query,
                limit,
                offset,
            } => self.search(query, limit, offset).await,
            LibraryCommand::Shutdown => Ok(()),
        }
    }

    async fn list_roots(&self) -> Result<()> {
        let roots: Vec<String> = sqlx::query_scalar!(
            r#"
            SELECT path
            FROM scan_roots
            WHERE enabled=1
            ORDER BY path
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        self.events.emit(LibraryEvent::Roots { paths: roots });
        Ok(())
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
        self.list_roots().await?;
        Ok(())
    }

    async fn remove_root(&self, path: String) -> Result<()> {
        sqlx::query!("UPDATE scan_roots SET enabled=0 WHERE path=?1", path)
            .execute(&self.pool)
            .await?;
        self.events.emit(LibraryEvent::Log {
            message: format!("disabled scan root: {}", path),
        });
        self.list_roots().await?;
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
                if let Some(old) = select_track_fingerprint(&self.pool, &file.path).await? {
                    if old.mtime_ms == file.mtime_ms
                        && old.size_bytes == file.size_bytes
                        && old.meta_scanned_ms > 0
                    {
                        skipped += 1;
                        continue;
                    }
                }

                let meta_scanned_ms = now_ms();

                let (title, artist, album, duration_ms, cover) =
                    match tokio::task::spawn_blocking({
                        let path = file.path.clone();
                        move || extract_metadata(Path::new(&path))
                    })
                    .await
                    {
                        Ok(Ok(m)) => (m.title, m.artist, m.album, m.duration_ms, m.cover),
                        Ok(Err(e)) => {
                            errors += 1;
                            self.events.emit(LibraryEvent::Log {
                                message: format!("metadata error: {}: {e:#}", file.path),
                            });
                            (None, None, None, None, None)
                        }
                        Err(join_err) => {
                            errors += 1;
                            self.events.emit(LibraryEvent::Log {
                                message: format!("metadata task failed: {}: {join_err}", file.path),
                            });
                            (None, None, None, None, None)
                        }
                    };

                let track_id = match upsert_track(
                    &self.pool,
                    &file.path,
                    &file.ext,
                    file.mtime_ms,
                    file.size_bytes,
                    title.as_deref(),
                    artist.as_deref(),
                    album.as_deref(),
                    duration_ms,
                    meta_scanned_ms,
                )
                .await
                {
                    Ok(v) => v,
                    Err(e) => {
                        errors += 1;
                        self.events.emit(LibraryEvent::Log {
                            message: format!("upsert error: {}: {e}", file.path),
                        });
                        continue;
                    }
                };

                if let Some(bytes) = cover {
                    if let Err(e) = write_cover_bytes(&self.cover_dir, track_id, &bytes) {
                        errors += 1;
                        self.events.emit(LibraryEvent::Log {
                            message: format!("cover write error: {}: {e}", file.path),
                        });
                    }
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
        let limit = limit.max(1).min(200);
        let offset = offset.max(0);

        let rows = if query.is_empty() {
            sqlx::query_as!(
                TrackLiteRow,
                r#"
                SELECT id, path, title, artist, album, duration_ms
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
        // Always quote tokens so that punctuation (e.g. apostrophes) won't break the FTS5 parser.
        .filter_map(|raw| {
            let token = raw
                .chars()
                .filter(|c| !c.is_control())
                .collect::<String>()
                .trim()
                .to_string();
            if token.is_empty() {
                return None;
            }

            // Escape double-quotes inside the token per SQLite rules.
            // See: https://www.sqlite.org/lang_expr.html (string literal escaping) and FTS5 query syntax.
            let token = token.replace('"', "\"\"");
            Some(format!("\"{token}\"*"))
        })
        .collect::<Vec<_>>()
        .join(" AND ")
}

#[cfg(test)]
mod tests {
    use super::build_fts_query;

    #[test]
    fn build_fts_query_quotes_tokens() {
        assert_eq!(build_fts_query("chu'meng"), "\"chu'meng\"*");
        assert_eq!(build_fts_query("hello world"), "\"hello\"* AND \"world\"*");
        assert_eq!(build_fts_query(r#"D:\CloudMusic"#), r#""D:\CloudMusic"*"#);
    }
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

#[derive(Debug, Clone, Copy)]
struct TrackFingerprint {
    mtime_ms: i64,
    size_bytes: i64,
    meta_scanned_ms: i64,
}

async fn select_track_fingerprint(
    pool: &SqlitePool,
    path: &str,
) -> Result<Option<TrackFingerprint>> {
    let row = sqlx::query!(
        "SELECT id, mtime_ms, size_bytes, meta_scanned_ms FROM tracks WHERE path=?1",
        path
    )
    .fetch_optional(pool)
    .await?;

    let Some(r) = row else { return Ok(None) };
    let _ = r.id.context("tracks.id is null")?;
    Ok(Some(TrackFingerprint {
        mtime_ms: r.mtime_ms,
        size_bytes: r.size_bytes,
        meta_scanned_ms: r.meta_scanned_ms,
    }))
}

async fn upsert_track(
    pool: &SqlitePool,
    path: &str,
    ext: &str,
    mtime_ms: i64,
    size_bytes: i64,
    title: Option<&str>,
    artist: Option<&str>,
    album: Option<&str>,
    duration_ms: Option<i64>,
    meta_scanned_ms: i64,
) -> Result<i64> {
    sqlx::query!(
        r#"
        INSERT INTO tracks(path, ext, mtime_ms, size_bytes, title, artist, album, duration_ms, meta_scanned_ms)
        VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        ON CONFLICT(path) DO UPDATE SET
            ext=excluded.ext,
            mtime_ms=excluded.mtime_ms,
            size_bytes=excluded.size_bytes,
            title=COALESCE(excluded.title, title),
            artist=COALESCE(excluded.artist, artist),
            album=COALESCE(excluded.album, album),
            duration_ms=COALESCE(excluded.duration_ms, duration_ms),
            meta_scanned_ms=excluded.meta_scanned_ms
        "#,
        path,
        ext,
        mtime_ms,
        size_bytes,
        title,
        artist,
        album,
        duration_ms,
        meta_scanned_ms
    )
    .execute(pool)
    .await?;

    let id: i64 = sqlx::query_scalar!("SELECT id FROM tracks WHERE path=?1", path)
        .fetch_one(pool)
        .await?
        .context("tracks.id is null")?;
    Ok(id)
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

#[derive(Default)]
struct ExtractedMetadata {
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    duration_ms: Option<i64>,
    cover: Option<Vec<u8>>,
}

fn extract_metadata(path: &Path) -> Result<ExtractedMetadata> {
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        hint.with_extension(ext);
    }

    let src = std::fs::File::open(path)
        .with_context(|| format!("failed to open for metadata: {}", path.display()))?;
    let mss = MediaSourceStream::new(Box::new(src), Default::default());

    // Allow reasonably-sized embedded artwork without blowing up memory usage.
    let meta_opts = MetadataOptions {
        limit_visual_bytes: Limit::Maximum(12 * 1024 * 1024),
        ..Default::default()
    };

    let mut probed = get_probe()
        .format(&hint, mss, &FormatOptions::default(), &meta_opts)
        .context("symphonia probe failed")?;

    let mut out = ExtractedMetadata::default();

    // Metadata read during probing (e.g. ID3 before container instantiation).
    if let Some(mut m) = probed.metadata.get() {
        if let Some(rev) = m.skip_to_latest() {
            apply_revision(rev, &mut out);
        }
    }

    // Metadata read from the container itself.
    {
        let mut m = probed.format.metadata();
        if let Some(rev) = m.skip_to_latest() {
            apply_revision(rev, &mut out);
        }
    }

    // Duration estimate from codec params (fast, no decoding).
    if let Some(track) = probed.format.default_track() {
        let cp = &track.codec_params;
        if let (Some(tb), Some(n_frames)) = (cp.time_base, cp.n_frames) {
            let t = tb.calc_time(n_frames);
            let ms = (t.seconds as f64 * 1000.0) + (t.frac * 1000.0);
            out.duration_ms = Some(ms.round() as i64);
        }
    }

    Ok(out)
}

fn apply_revision(rev: &symphonia::core::meta::MetadataRevision, out: &mut ExtractedMetadata) {
    for tag in rev.tags() {
        if out.title.is_none() && matches!(tag.std_key, Some(StandardTagKey::TrackTitle)) {
            out.title = value_to_string(&tag.value);
            continue;
        }
        if out.artist.is_none() && matches!(tag.std_key, Some(StandardTagKey::Artist)) {
            out.artist = value_to_string(&tag.value);
            continue;
        }
        if out.album.is_none() && matches!(tag.std_key, Some(StandardTagKey::Album)) {
            out.album = value_to_string(&tag.value);
            continue;
        }

        // Fallback for readers that don't assign std_key.
        if tag.std_key.is_none() {
            let key = tag.key.trim().to_ascii_lowercase();
            match key.as_str() {
                "title" | "tracktitle" => {
                    if out.title.is_none() {
                        out.title = value_to_string(&tag.value);
                    }
                }
                "artist" => {
                    if out.artist.is_none() {
                        out.artist = value_to_string(&tag.value);
                    }
                }
                "album" => {
                    if out.album.is_none() {
                        out.album = value_to_string(&tag.value);
                    }
                }
                _ => {}
            }
        }
    }

    if out.cover.is_none() {
        let front = rev
            .visuals()
            .iter()
            .find(|v| v.usage == Some(StandardVisualKey::FrontCover));
        let any = rev.visuals().first();
        let chosen = front.or(any);
        if let Some(v) = chosen {
            if !v.data.is_empty() {
                out.cover = Some(v.data.as_ref().to_vec());
            }
        }
    }
}

fn value_to_string(v: &Value) -> Option<String> {
    let s = match v {
        Value::String(s) => s.clone(),
        _ => v.to_string(),
    };
    let s = s.trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

fn write_cover_bytes(cover_dir: &Path, track_id: i64, bytes: &[u8]) -> Result<()> {
    if bytes.is_empty() {
        return Ok(());
    }

    std::fs::create_dir_all(cover_dir)
        .with_context(|| format!("failed to create cover dir: {}", cover_dir.display()))?;

    let final_path = cover_dir.join(track_id.to_string());
    let tmp_path = cover_dir.join(format!("{}.tmp", track_id));
    std::fs::write(&tmp_path, bytes)
        .with_context(|| format!("failed to write cover temp: {}", tmp_path.display()))?;

    // Best-effort atomic replace.
    let _ = std::fs::remove_file(&final_path);
    std::fs::rename(&tmp_path, &final_path)
        .with_context(|| format!("failed to rename cover: {}", final_path.display()))?;

    Ok(())
}
