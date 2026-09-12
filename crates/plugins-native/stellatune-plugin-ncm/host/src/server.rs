use crate::ncm::{NcmContainer, NcmSource};
use anyhow::{Result, ensure};
use axum::{
    Router,
    body::{Body, Bytes},
    extract::{Path, State},
    http::{HeaderMap, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    io::{Read, Seek, SeekFrom},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::SystemTime,
};

#[derive(Clone)]
struct Entry {
    path: PathBuf,
    size: u64,
    modified: SystemTime,
    cover: Option<(u64, u32)>,
}

#[derive(Default)]
pub struct Sources {
    entries: HashMap<u64, Entry>,
    paths: HashMap<PathBuf, u64>,
    next_id: u64,
}

type Shared = Arc<Mutex<Sources>>;

pub fn command(state: &Shared, base: &str, request: &Value) -> Result<Value> {
    let path = std::fs::canonicalize(
        request["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("path required"))?,
    )?;
    ensure!(
        matches!(
            request["operation"].as_str(),
            Some("inspect-file" | "resolve-file")
        ),
        "unsupported NCM operation"
    );
    let metadata = std::fs::metadata(&path)?;
    let mut audio = None;
    let mut audio_probe_status = stellatune_media_probe::ProbeStatus::Unsupported;
    if request["operation"] == "inspect-file" && request["skipAudio"] != true {
        // Inspect the decrypted payload directly. No HTTP, temporary file or album-sized buffer.
        let opened = NcmSource::open(&path);
        if let Ok(mut source) = opened {
            let hint = source.info.format.clone();
            if let Ok(reader) = stellatune_media_probe::slice::AudioSlice::new(
                &mut source.reader,
                source.start,
                source.length,
            ) {
                let result = stellatune_media_probe::probe(
                    reader,
                    Some(&hint),
                    &std::sync::atomic::AtomicBool::new(false),
                );
                audio_probe_status = result.status;
                audio = result.properties.map(|mut p| {
                    p.format = Some("NCM".into());
                    p
                });
            } else {
                audio_probe_status = stellatune_media_probe::ProbeStatus::IoError;
            }
        } else if let Err(error) = opened {
            audio_probe_status = if error.downcast_ref::<std::io::Error>().is_some() {
                stellatune_media_probe::ProbeStatus::IoError
            } else {
                stellatune_media_probe::ProbeStatus::Invalid
            };
        }
        if request["propertiesOnly"] == true {
            let after = std::fs::metadata(&path)?;
            ensure!(
                metadata.len() == after.len() && metadata.modified()? == after.modified()?,
                "NCM file changed during inspection"
            );
            return Ok(json!({ "audio": audio, "audioProbeStatus": audio_probe_status }));
        }
    }
    let (info, cover) = if request["operation"] == "inspect-file" {
        let container = NcmContainer::open(&path)?;
        (container.info, container.cover)
    } else {
        let source = NcmSource::open(&path)?;
        (source.info, source.cover)
    };
    let after = std::fs::metadata(&path)?;
    ensure!(
        metadata.len() == after.len() && metadata.modified()? == after.modified()?,
        "NCM file changed during inspection"
    );
    let mut state = state.lock().unwrap();
    let id = if let Some(id) = state.paths.get(&path).copied()
        && let Some(entry) = state.entries.get(&id)
        && entry.size == metadata.len()
        && entry.modified == metadata.modified()?
    {
        id
    } else {
        state.next_id += 1;
        let id = state.next_id;
        if let Some(old) = state.paths.insert(path.clone(), id) {
            state.entries.remove(&old);
        }
        state.entries.insert(
            id,
            Entry {
                path,
                size: metadata.len(),
                modified: metadata.modified()?,
                cover,
            },
        );
        id
    };

    match request["operation"].as_str() {
        Some("inspect-file") => Ok(json!({
            "audio": audio,
            "audioProbeStatus": audio_probe_status,
            "title": info.name,
            "artist": info.artist.iter().map(|row| row.0.as_str()).collect::<Vec<_>>().join(" / "),
            "album": info.album,
            "durationMs": info.duration,
            "coverUrl": cover.map(|_| format!("{base}/cover/{id}")),
        })),
        Some("resolve-file") => Ok(json!({
            "source": {"kind": "http", "url": format!("{base}/audio/{id}"), "headers": {}},
            "media": {"codecHint": info.format},
            "capabilities": {"seekable": true, "live": false, "durationMs": info.duration}
        })),
        _ => anyhow::bail!("unsupported NCM operation"),
    }
}

pub fn router(state: Shared) -> Router {
    Router::new()
        .route("/audio/{id}", get(audio))
        .route("/cover/{id}", get(cover))
        .with_state(state)
}

async fn cover(State(state): State<Shared>, Path(id): Path<u64>) -> Response {
    let Some(entry) = state.lock().unwrap().entries.get(&id).cloned() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Some((offset, length)) = entry.cover else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let read = tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
        let mut file = std::fs::File::open(&entry.path)?;
        let metadata = file.metadata()?;
        ensure!(
            metadata.len() == entry.size && metadata.modified()? == entry.modified,
            "NCM file changed; inspect it again"
        );
        file.seek(SeekFrom::Start(offset))?;
        let mut bytes = vec![0; length as usize];
        file.read_exact(&mut bytes)?;
        Ok(bytes)
    })
    .await;
    match read {
        Ok(Ok(bytes)) => (
            [
                ("content-type", "application/octet-stream"),
                ("cache-control", "no-store"),
            ],
            bytes,
        )
            .into_response(),
        _ => StatusCode::CONFLICT.into_response(),
    }
}

async fn audio(
    State(state): State<Shared>,
    Path(id): Path<u64>,
    method: Method,
    headers: HeaderMap,
) -> Response {
    let Some(entry) = state.lock().unwrap().entries.get(&id).cloned() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let opened = tokio::task::spawn_blocking(move || -> Result<NcmSource> {
        let metadata = std::fs::metadata(&entry.path)?;
        ensure!(
            metadata.len() == entry.size && metadata.modified()? == entry.modified,
            "NCM file changed; resolve it again"
        );
        NcmSource::open(&entry.path)
    })
    .await;
    let mut source = match opened {
        Ok(Ok(source)) => source,
        _ => return StatusCode::CONFLICT.into_response(),
    };
    let length = source.length;
    let range = match headers.get("range") {
        Some(value) => match value
            .to_str()
            .ok()
            .and_then(|value| parse_range(value, length))
        {
            Some(range) => Some(range),
            None => {
                return (
                    StatusCode::RANGE_NOT_SATISFIABLE,
                    [("content-range", format!("bytes */{length}"))],
                )
                    .into_response();
            },
        },
        None => None,
    };
    let (start, end) = range.unwrap_or((0, length - 1));
    let count = end - start + 1;
    let mime = if source.info.format == "flac" {
        "audio/flac"
    } else {
        "audio/mpeg"
    };
    let body = if method == Method::HEAD {
        Body::empty()
    } else {
        let (tx, rx) = tokio::sync::mpsc::channel(2);
        tokio::task::spawn_blocking(move || {
            let result = (|| -> std::io::Result<()> {
                source.reader.seek(SeekFrom::Start(source.start + start))?;
                let mut remaining = count;
                while remaining > 0 && !tx.is_closed() {
                    let mut buffer = vec![0; remaining.min(64 * 1024) as usize];
                    source.reader.read_exact(&mut buffer)?;
                    remaining -= buffer.len() as u64;
                    if tx
                        .blocking_send(Ok::<_, std::io::Error>(Bytes::from(buffer)))
                        .is_err()
                    {
                        break;
                    }
                }
                Ok(())
            })();
            if let Err(error) = result {
                let _ = tx.blocking_send(Err(error));
            }
        });
        Body::from_stream(futures_util::stream::unfold(rx, |mut rx| async {
            rx.recv().await.map(|value| (value, rx))
        }))
    };
    let mut response = Response::builder()
        .status(if range.is_some() {
            StatusCode::PARTIAL_CONTENT
        } else {
            StatusCode::OK
        })
        .header("accept-ranges", "bytes")
        .header("content-type", mime)
        .header("content-length", count)
        .header("cache-control", "no-store");
    if range.is_some() {
        response = response.header("content-range", format!("bytes {start}-{end}/{length}"));
    }
    response.body(body).unwrap()
}

fn parse_range(value: &str, length: u64) -> Option<(u64, u64)> {
    let (start, end) = value.strip_prefix("bytes=")?.split_once('-')?;
    if length == 0 {
        return None;
    }
    if start.is_empty() {
        let suffix: u64 = end.parse().ok()?;
        return (suffix > 0).then_some((length.saturating_sub(suffix), length - 1));
    }
    let start: u64 = start.parse().ok()?;
    let end = if end.is_empty() {
        length - 1
    } else {
        end.parse::<u64>().ok()?.min(length - 1)
    };
    (start < length && start <= end).then_some((start, end))
}

#[cfg(test)]
mod tests {
    use super::parse_range;
    #[test]
    #[ignore = "Read-only user supplied NCM file"]
    fn real_ncm_properties() {
        let path = std::env::var("STELLATUNE_NCM_AUDIT").unwrap();
        let state = std::sync::Arc::new(std::sync::Mutex::new(super::Sources::default()));
        let result = super::command(
            &state,
            "http://127.0.0.1",
            &serde_json::json!({"operation":"inspect-file","path":path,"propertiesOnly":true}),
        )
        .unwrap();
        println!("{path}: {result}");
        assert_eq!(result["audioProbeStatus"], "ready");
        assert!(result["audio"]["bitrate"]["bps"].as_u64().unwrap() > 0);
        assert!(state.lock().unwrap().entries.is_empty());
    }
    #[test]
    fn ncm_properties_match_plain_payload_without_creating_files() {
        use std::io::Read;
        for name in ["tone.ncm", "tone-mp3.ncm", "tone-cover.ncm"] {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../tests/fixtures")
                .join(name);
            let original = std::fs::read(&path).unwrap();
            let mut source = crate::ncm::NcmSource::open(&path).unwrap();
            let mut slice = stellatune_media_probe::slice::AudioSlice::new(
                &mut source.reader,
                source.start,
                source.length,
            )
            .unwrap();
            let mut payload = Vec::new();
            slice.read_to_end(&mut payload).unwrap();
            let plain = stellatune_media_probe::probe(
                std::io::Cursor::new(payload),
                None,
                &std::sync::atomic::AtomicBool::new(false),
            )
            .properties
            .unwrap();
            let state = std::sync::Arc::new(std::sync::Mutex::new(super::Sources::default()));
            let result = super::command(
                &state,
                "http://127.0.0.1",
                &serde_json::json!({"operation":"inspect-file","path":path,"propertiesOnly":true}),
            )
            .unwrap();
            let audio: stellatune_media_probe::AudioProperties =
                serde_json::from_value(result["audio"].clone()).unwrap();
            assert_eq!(audio.format.as_deref(), Some("NCM"));
            assert_eq!(audio.codec, plain.codec);
            assert_eq!(audio.bitrate, plain.bitrate);
            assert_eq!(audio.sample_rate, plain.sample_rate);
            assert_eq!(audio.bits_per_sample, plain.bits_per_sample);
            assert!(
                state.lock().unwrap().entries.is_empty(),
                "property requests must not publish HTTP resources"
            );
            assert_eq!(std::fs::read(&path).unwrap(), original);
        }
    }
    #[test]
    fn byte_ranges_cover_bounded_open_suffix_and_invalid_requests() {
        assert_eq!(parse_range("bytes=3-7", 10), Some((3, 7)));
        assert_eq!(parse_range("bytes=3-", 10), Some((3, 9)));
        assert_eq!(parse_range("bytes=-4", 10), Some((6, 9)));
        assert_eq!(parse_range("bytes=3-100", 10), Some((3, 9)));
        for value in [
            "bytes=10-",
            "bytes=-0",
            "bytes=7-2",
            "bytes=0-1,3-4",
            "other=0-1",
        ] {
            assert_eq!(parse_range(value, 10), None);
        }
    }
}
