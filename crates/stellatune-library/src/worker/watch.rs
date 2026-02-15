use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use sqlx::SqlitePool;
use tokio::sync::mpsc;
use tokio::time::Instant;

use crate::LibraryEvent;
use stellatune_runtime::tokio_actor::ActorRef;

use crate::service::EventHub;

use super::metadata::{
    extract_metadata_with_plugins, has_plugin_decoder_for_path, write_cover_bytes,
};
use super::paths::{is_under_excluded, normalize_path_str, now_ms, parent_dir_norm};
use super::tracks::{
    UpsertTrackInput, delete_track_by_path_norm, select_track_fingerprint_by_path_norm,
    upsert_track_by_path_norm,
};

pub(super) mod handlers;

use self::handlers::fs_event::WatchFsEventMessage;
use self::handlers::refresh::WatchRefreshMessage;
use self::handlers::tick::WatchTickMessage;

const WATCH_DEBOUNCE_MS: u64 = 750;
const WATCH_TICK_MS: u64 = 100;

pub(super) struct WatchTaskActor {
    pool: SqlitePool,
    events: Arc<EventHub>,
    cover_dir: PathBuf,
    watcher: Option<RecommendedWatcher>,
    watched: HashSet<String>,
    excluded: Vec<String>,
    dirty: HashSet<String>,
    debounce_deadline: Option<Instant>,
}

async fn refresh_watch_state(
    pool: &SqlitePool,
    watcher: &mut RecommendedWatcher,
    watched: &mut HashSet<String>,
    excluded: &mut Vec<String>,
) -> Result<()> {
    let roots: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT path
        FROM scan_roots
        WHERE enabled=1
        "#,
    )
    .fetch_all(pool)
    .await?;

    let desired = roots
        .into_iter()
        .map(|p| normalize_path_str(&p))
        .filter(|p| !p.is_empty())
        .collect::<HashSet<_>>();

    *excluded = sqlx::query_scalar("SELECT path FROM excluded_folders")
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|p: String| normalize_path_str(&p))
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>();

    for r in desired.iter() {
        if watched.contains(r) {
            continue;
        }
        watcher
            .watch(Path::new(r), RecursiveMode::Recursive)
            .with_context(|| format!("watch failed: {r}"))?;
        watched.insert(r.clone());
    }

    let to_remove = watched
        .iter()
        .filter(|p| !desired.contains(*p))
        .cloned()
        .collect::<Vec<_>>();
    for r in to_remove {
        let _ = watcher.unwatch(Path::new(&r));
        watched.remove(&r);
    }

    Ok(())
}

pub(super) fn spawn_watch_task(
    pool: SqlitePool,
    events: Arc<EventHub>,
    cover_dir: PathBuf,
) -> ActorRef<WatchTaskActor> {
    let (fs_tx, mut fs_rx) = mpsc::unbounded_channel::<notify::Result<notify::Event>>();
    let watcher = match notify::recommended_watcher(move |res| {
        let _ = fs_tx.send(res);
    }) {
        Ok(w) => Some(w),
        Err(err) => {
            events.emit(LibraryEvent::Log {
                message: format!("fs watcher init failed: {err}"),
            });
            None
        }
    };

    let has_watcher = watcher.is_some();
    let (actor_ref, _join) = stellatune_runtime::tokio_actor::spawn_actor(WatchTaskActor {
        pool,
        events: Arc::clone(&events),
        cover_dir,
        watcher,
        watched: HashSet::new(),
        excluded: Vec::new(),
        dirty: HashSet::new(),
        debounce_deadline: None,
    });

    let _ = actor_ref.cast(WatchRefreshMessage);

    if has_watcher {
        let fs_actor_ref = actor_ref.clone();
        stellatune_runtime::spawn(async move {
            while let Some(result) = fs_rx.recv().await {
                if fs_actor_ref.cast(WatchFsEventMessage { result }).is_err() {
                    break;
                }
            }
        });
    }

    let tick_actor_ref = actor_ref.clone();
    stellatune_runtime::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(WATCH_TICK_MS));
        loop {
            interval.tick().await;
            if tick_actor_ref.cast(WatchTickMessage).is_err() {
                break;
            }
        }
    });

    actor_ref
}

pub(super) fn request_watch_refresh(actor_ref: &ActorRef<WatchTaskActor>) {
    let _ = actor_ref.cast(WatchRefreshMessage);
}

fn is_audio_ext(ext: &str) -> bool {
    matches!(ext, "mp3" | "flac" | "wav")
}

async fn apply_fs_changes(
    pool: &SqlitePool,
    events: &Arc<EventHub>,
    cover_dir: &Path,
    excluded: &[String],
    raw_paths: Vec<String>,
) -> Result<bool> {
    let mut changed = false;

    for raw in raw_paths {
        let raw_trimmed = raw.trim();
        if raw_trimmed.is_empty() {
            continue;
        }

        let path = PathBuf::from(raw_trimmed);
        let path_norm = normalize_path_str(raw_trimmed);
        if path_norm.is_empty() {
            continue;
        }

        if std::fs::metadata(&path).ok().is_some_and(|m| m.is_dir()) {
            continue;
        }

        let dir_norm = parent_dir_norm(&path_norm).unwrap_or_default();
        if !dir_norm.is_empty() && is_under_excluded(&dir_norm, excluded) {
            continue;
        }

        let meta = match std::fs::metadata(&path) {
            Ok(m) => m,
            Err(_) => {
                let deleted = delete_track_by_path_norm(pool, cover_dir, &path_norm).await?;
                changed |= deleted > 0;
                continue;
            }
        };

        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let supported = is_audio_ext(&ext) || has_plugin_decoder_for_path(&path);
        if !supported {
            let deleted = delete_track_by_path_norm(pool, cover_dir, &path_norm).await?;
            changed |= deleted > 0;
            continue;
        }

        let mtime_ms = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let size_bytes = meta.len() as i64;

        if let Some(old) = select_track_fingerprint_by_path_norm(pool, &path_norm).await?
            && old.mtime_ms == mtime_ms
            && old.size_bytes == size_bytes
            && old.meta_scanned_ms > 0
        {
            continue;
        }

        // Heavy metadata extraction happens only when the fingerprint differs.
        let meta_scanned_ms = now_ms();

        let (title, artist, album, duration_ms, cover) = match tokio::task::spawn_blocking({
            let p = path.clone();
            move || {
                extract_metadata_with_plugins(&p)
                    .map(|m| (m.title, m.artist, m.album, m.duration_ms, m.cover))
            }
        })
        .await
        {
            Ok(Ok(m)) => m,
            Ok(Err(e)) => {
                events.emit(LibraryEvent::Log {
                    message: format!("metadata error: {}: {e:#}", raw_trimmed),
                });
                (None, None, None, None, None)
            }
            Err(join_err) => {
                events.emit(LibraryEvent::Log {
                    message: format!("metadata task failed: {}: {join_err}", raw_trimmed),
                });
                (None, None, None, None, None)
            }
        };

        let track_id = upsert_track_by_path_norm(
            pool,
            UpsertTrackInput {
                path: raw_trimmed,
                ext: &ext,
                mtime_ms,
                size_bytes,
                title: title.as_deref(),
                artist: artist.as_deref(),
                album: album.as_deref(),
                duration_ms,
                meta_scanned_ms,
                path_norm: &path_norm,
                dir_norm: &dir_norm,
            },
        )
        .await?;

        if let Some(bytes) = cover
            && let Err(e) = write_cover_bytes(cover_dir, track_id, &bytes)
        {
            events.emit(LibraryEvent::Log {
                message: format!("cover write error: {}: {e}", raw_trimmed),
            });
        }

        changed = true;
    }

    Ok(changed)
}
