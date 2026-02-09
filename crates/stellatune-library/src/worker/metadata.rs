use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use base64::Engine;
use stellatune_plugin_api::{
    ST_DECODER_INFO_FLAG_HAS_DURATION, ST_ERR_INVALID_ARG, ST_ERR_IO, StIoVTable, StSeekWhence,
    StStatus, StStr,
};
use stellatune_plugins::runtime::CapabilityKind as RuntimeCapabilityKind;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::{Limit, MetadataOptions, StandardTagKey, StandardVisualKey, Value};
use symphonia::core::probe::Hint;
use symphonia::default::get_probe;
use tracing::debug;

#[derive(Debug, serde::Deserialize)]
struct PluginTrackMetadata {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    artist: Option<String>,
    #[serde(default)]
    album: Option<String>,
    #[serde(default)]
    duration_ms: Option<i64>,
    #[serde(default)]
    cover_base64: Option<String>,
    #[serde(default)]
    cover_bytes: Option<Vec<u8>>,
}

#[derive(Default)]
pub(super) struct ExtractedMetadata {
    pub(super) title: Option<String>,
    pub(super) artist: Option<String>,
    pub(super) album: Option<String>,
    pub(super) duration_ms: Option<i64>,
    pub(super) cover: Option<Vec<u8>>,
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
#[derive(Debug, Clone)]
struct DecoderCandidate {
    plugin_id: String,
    type_id: String,
    default_config_json: String,
    score: u16,
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
struct LocalFileIoHandle {
    file: File,
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
fn status_code(code: i32) -> StStatus {
    StStatus {
        code,
        message: StStr::empty(),
    }
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
extern "C" fn local_io_read(
    handle: *mut core::ffi::c_void,
    out: *mut u8,
    len: usize,
    out_read: *mut usize,
) -> StStatus {
    if handle.is_null() || out_read.is_null() || (len > 0 && out.is_null()) {
        return status_code(ST_ERR_INVALID_ARG);
    }
    let state = unsafe { &mut *(handle as *mut LocalFileIoHandle) };
    let out_slice: &mut [u8] = if len == 0 {
        &mut []
    } else {
        unsafe { core::slice::from_raw_parts_mut(out, len) }
    };
    match state.file.read(out_slice) {
        Ok(n) => {
            unsafe {
                *out_read = n;
            }
            StStatus::ok()
        }
        Err(_) => status_code(ST_ERR_IO),
    }
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
extern "C" fn local_io_seek(
    handle: *mut core::ffi::c_void,
    offset: i64,
    whence: StSeekWhence,
    out_pos: *mut u64,
) -> StStatus {
    if handle.is_null() || out_pos.is_null() {
        return status_code(ST_ERR_INVALID_ARG);
    }
    let state = unsafe { &mut *(handle as *mut LocalFileIoHandle) };
    let seek_from = match whence {
        StSeekWhence::Start => {
            if offset < 0 {
                return status_code(ST_ERR_INVALID_ARG);
            }
            SeekFrom::Start(offset as u64)
        }
        StSeekWhence::Current => SeekFrom::Current(offset),
        StSeekWhence::End => SeekFrom::End(offset),
    };
    match state.file.seek(seek_from) {
        Ok(pos) => {
            unsafe {
                *out_pos = pos;
            }
            StStatus::ok()
        }
        Err(_) => status_code(ST_ERR_IO),
    }
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
extern "C" fn local_io_tell(handle: *mut core::ffi::c_void, out_pos: *mut u64) -> StStatus {
    if handle.is_null() || out_pos.is_null() {
        return status_code(ST_ERR_INVALID_ARG);
    }
    let state = unsafe { &mut *(handle as *mut LocalFileIoHandle) };
    match state.file.stream_position() {
        Ok(pos) => {
            unsafe {
                *out_pos = pos;
            }
            StStatus::ok()
        }
        Err(_) => status_code(ST_ERR_IO),
    }
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
extern "C" fn local_io_size(handle: *mut core::ffi::c_void, out_size: *mut u64) -> StStatus {
    if handle.is_null() || out_size.is_null() {
        return status_code(ST_ERR_INVALID_ARG);
    }
    let state = unsafe { &mut *(handle as *mut LocalFileIoHandle) };
    match state.file.metadata() {
        Ok(meta) => {
            unsafe {
                *out_size = meta.len();
            }
            StStatus::ok()
        }
        Err(_) => status_code(ST_ERR_IO),
    }
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
static LOCAL_FILE_IO_VTABLE: StIoVTable = StIoVTable {
    read: local_io_read,
    seek: Some(local_io_seek),
    tell: Some(local_io_tell),
    size: Some(local_io_size),
};

pub(super) fn extract_metadata(path: &Path) -> Result<ExtractedMetadata> {
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        hint.with_extension(ext);
    }

    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    debug!(
        target: "stellatune_library::metadata",
        path = %path.display(),
        ext = %ext,
        "symphonia metadata probe begin"
    );

    let src = std::fs::File::open(path)
        .with_context(|| format!("failed to open for metadata: {}", path.display()))?;
    let mss = MediaSourceStream::new(Box::new(src), Default::default());

    // Allow reasonably-sized embedded artwork without blowing up memory usage.
    let meta_opts = MetadataOptions {
        limit_visual_bytes: Limit::Maximum(12 * 1024 * 1024),
        ..Default::default()
    };

    let mut probed = match get_probe().format(&hint, mss, &FormatOptions::default(), &meta_opts) {
        Ok(p) => p,
        Err(e) => {
            let (file_size, head16) = {
                let file_size = std::fs::metadata(path).ok().map(|m| m.len());
                let mut head16 = [0u8; 16];
                if let Ok(mut f) = std::fs::File::open(path) {
                    use std::io::Read as _;
                    let _ = f.read(&mut head16);
                }
                (file_size, head16)
            };

            debug!(
                target: "stellatune_library::metadata",
                path = %path.display(),
                ext = %ext,
                file_size = file_size.unwrap_or(0),
                head16 = ?head16,
                err = %e,
                "symphonia metadata probe failed"
            );

            return Err(e).context("symphonia probe failed");
        }
    };

    let mut out = ExtractedMetadata::default();

    // Metadata read during probing (e.g. ID3 before container instantiation).
    if let Some(mut m) = probed.metadata.get()
        && let Some(rev) = m.skip_to_latest()
    {
        apply_revision(rev, &mut out);
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

    if out.cover.is_none() {
        out.cover = load_sidecar_cover(path);
    }

    debug!(
        target: "stellatune_library::metadata",
        path = %path.display(),
        title = out.title.as_deref().unwrap_or(""),
        artist = out.artist.as_deref().unwrap_or(""),
        album = out.album.as_deref().unwrap_or(""),
        duration_ms = out.duration_ms.unwrap_or(-1),
        cover = out.cover.as_ref().map(|b| b.len()).unwrap_or(0),
        "symphonia metadata probe ok"
    );

    Ok(out)
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
fn normalize_ext_hint(raw: &str) -> String {
    raw.trim().trim_start_matches('.').to_ascii_lowercase()
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
fn decoder_candidates_for_ext(ext: &str) -> Vec<DecoderCandidate> {
    let normalized = normalize_ext_hint(ext);
    if normalized.is_empty() {
        return Vec::new();
    }
    let shared = stellatune_plugins::shared_runtime_service();
    let Ok(service) = shared.lock() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for candidate in service.decoder_candidates_for_ext(&normalized) {
        let Some(cap) = service.resolve_active_capability(
            &candidate.plugin_id,
            RuntimeCapabilityKind::Decoder,
            &candidate.type_id,
        ) else {
            continue;
        };
        out.push(DecoderCandidate {
            plugin_id: candidate.plugin_id,
            type_id: candidate.type_id,
            default_config_json: cap.default_config_json,
            score: candidate.score,
        });
    }
    out
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
fn best_decoder_score_for_ext(ext: &str) -> Option<u16> {
    decoder_candidates_for_ext(ext)
        .into_iter()
        .map(|v| v.score)
        .max()
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
fn select_plugin_metadata_decoder_candidates(path: &Path) -> Vec<DecoderCandidate> {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    decoder_candidates_for_ext(&ext)
}

pub(super) fn has_plugin_decoder_for_path(path: &Path) -> bool {
    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
    {
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if ext.is_empty() {
            return false;
        }
        return !decoder_candidates_for_ext(&ext).is_empty();
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        let _ = path;
        false
    }
}

pub(super) fn extract_metadata_with_plugins(path: &Path) -> Result<ExtractedMetadata> {
    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
    {
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();

        const BUILTIN_META_SCORE: u16 = 50;
        let prefer_plugin = if ext.is_empty() {
            false
        } else if is_symphonia_primary_ext(&ext) {
            best_decoder_score_for_ext(&ext).is_some_and(|score| score > BUILTIN_META_SCORE)
        } else {
            best_decoder_score_for_ext(&ext).is_some_and(|score| score > 0)
        };

        if prefer_plugin {
            debug!(
                target: "stellatune_library::metadata",
                path = %path.display(),
                ext = %ext,
                "using v2 plugin metadata extractor"
            );
            return extract_plugin_metadata_from_plugin(path);
        }
    }

    extract_metadata(path)
}

fn is_symphonia_primary_ext(ext_lower: &str) -> bool {
    matches!(ext_lower, "mp3" | "flac" | "wav")
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
fn extract_plugin_metadata_from_plugin(path: &Path) -> Result<ExtractedMetadata> {
    let started = std::time::Instant::now();
    let path_str = path.to_string_lossy().to_string();
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let candidates = select_plugin_metadata_decoder_candidates(path);
    if candidates.is_empty() {
        return Err(anyhow::anyhow!(
            "no v2 plugin decoder candidate for {}",
            path.display()
        ));
    }

    let mut last_err: Option<String> = None;
    for candidate in candidates {
        let shared = stellatune_plugins::shared_runtime_service();
        let mut dec = match shared.lock() {
            Ok(service) => match service.create_decoder_instance(
                &candidate.plugin_id,
                &candidate.type_id,
                &candidate.default_config_json,
            ) {
                Ok(v) => v,
                Err(e) => {
                    last_err = Some(format!(
                        "create_decoder_instance failed for {}::{}: {e:#}",
                        candidate.plugin_id, candidate.type_id
                    ));
                    continue;
                }
            },
            Err(_) => {
                last_err = Some("runtime service mutex poisoned".to_string());
                continue;
            }
        };

        let mut file = match File::open(path) {
            Ok(file) => Box::new(LocalFileIoHandle { file }),
            Err(e) => {
                return Err(e)
                    .with_context(|| format!("failed to open for metadata: {}", path.display()));
            }
        };
        let io_handle = (&mut *file) as *mut LocalFileIoHandle as *mut core::ffi::c_void;
        if let Err(e) = dec.open_with_io(
            &path_str,
            &ext,
            &LOCAL_FILE_IO_VTABLE as *const _,
            io_handle,
        ) {
            last_err = Some(format!(
                "decoder open_with_io failed for {}::{}: {e:#}",
                candidate.plugin_id, candidate.type_id
            ));
            continue;
        }

        debug!(
            target: "stellatune_library::metadata",
            path = %path.display(),
            plugin_id = %candidate.plugin_id,
            decoder_type_id = %candidate.type_id,
            elapsed_ms = started.elapsed().as_millis(),
            "v2 plugin decoder opened for metadata"
        );

        let info = dec.get_info().map_err(|e| anyhow::anyhow!("{e:#}"))?;
        let mut out = ExtractedMetadata {
            duration_ms: if info.flags & ST_DECODER_INFO_FLAG_HAS_DURATION != 0 {
                Some(info.duration_ms as i64)
            } else {
                None
            },
            ..Default::default()
        };

        if let Ok(Some(raw)) = dec.get_metadata_json()
            && let Ok(meta) = serde_json::from_str::<PluginTrackMetadata>(&raw)
        {
            out.title = meta
                .title
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            out.artist = meta
                .artist
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            out.album = meta
                .album
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());

            if out.duration_ms.is_none() {
                out.duration_ms = meta.duration_ms.filter(|ms| *ms >= 0);
            }

            if out.cover.is_none() {
                if let Some(s) = meta.cover_base64.as_deref() {
                    let s = s.trim();
                    if !s.is_empty()
                        && let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(s)
                        && !bytes.is_empty()
                        && (bytes.len() as u64) <= COVER_BYTES_LIMIT
                    {
                        out.cover = Some(bytes);
                    }
                } else if let Some(bytes) = meta.cover_bytes
                    && !bytes.is_empty()
                    && (bytes.len() as u64) <= COVER_BYTES_LIMIT
                {
                    out.cover = Some(bytes);
                }
            }
        }

        if out.cover.is_none() {
            out.cover = load_sidecar_cover(path);
        }
        return Ok(out);
    }

    Err(anyhow::anyhow!(
        "failed to extract plugin metadata for {}: {}",
        path.display(),
        last_err.unwrap_or_else(|| "no decoder candidate succeeded".to_string())
    ))
}

const COVER_BYTES_LIMIT: u64 = 12 * 1024 * 1024;

#[derive(Default)]
struct DirImageIndex {
    // Lowercased stem -> image path
    by_stem: HashMap<String, PathBuf>,
    preferred_candidates: Vec<PathBuf>,
    preferred_bytes: Option<Vec<u8>>,
}

thread_local! {
    static DIR_IMAGE_INDEX: std::cell::RefCell<HashMap<PathBuf, DirImageIndex>> =
        std::cell::RefCell::new(HashMap::new());
}

fn load_sidecar_cover(track_path: &Path) -> Option<Vec<u8>> {
    let dir = track_path.parent()?.to_path_buf();
    let stem = track_path.file_stem()?.to_string_lossy().to_string();
    let stem_key = stem.trim().to_ascii_lowercase();
    if stem_key.is_empty() {
        return None;
    }

    DIR_IMAGE_INDEX.with_borrow_mut(|cache| {
        // Simple cap to avoid unbounded growth during long scans.
        if cache.len() > 256 {
            cache.clear();
        }

        let idx = cache
            .entry(dir.clone())
            .or_insert_with(|| build_dir_index(&dir));

        if let Some(bytes) = idx
            .by_stem
            .get(&stem_key)
            .cloned()
            .and_then(|p| read_cover_bytes(&p))
        {
            return Some(bytes);
        }

        if let Some(bytes) = idx.preferred_bytes.as_ref() {
            return Some(bytes.clone());
        }

        for p in idx.preferred_candidates.iter() {
            if let Some(bytes) = read_cover_bytes(p) {
                idx.preferred_bytes = Some(bytes.clone());
                return Some(bytes);
            }
        }

        None
    })
}

fn build_dir_index(dir: &Path) -> DirImageIndex {
    let mut out = DirImageIndex::default();

    let mut images: Vec<(String, PathBuf, u64)> = Vec::new(); // stem_lower, path, size
    let rd = match std::fs::read_dir(dir) {
        Ok(v) => v,
        Err(_) => return out,
    };

    for entry in rd.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !matches!(
            ext.as_str(),
            "jpg" | "jpeg" | "png" | "webp" | "bmp" | "gif"
        ) {
            continue;
        }

        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        if stem.is_empty() {
            continue;
        }

        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        images.push((stem, path, size));
    }

    fn preferred_score(stem: &str) -> Option<u8> {
        let s = stem;
        if s == "cover" {
            return Some(0);
        }
        if s.starts_with("cover") {
            return Some(1);
        }
        if s == "folder" {
            return Some(2);
        }
        if s.starts_with("folder") {
            return Some(3);
        }
        if s == "front" {
            return Some(4);
        }
        if s.starts_with("front") {
            return Some(5);
        }
        if s == "album" {
            return Some(6);
        }
        if s.starts_with("album") {
            return Some(7);
        }
        if s.contains("albumart") {
            return Some(8);
        }
        if s.contains("artwork") {
            return Some(9);
        }
        None
    }

    // For each stem, keep the smallest file (usually the intended cover, and cheaper to load).
    let mut best: HashMap<String, (PathBuf, u64)> = HashMap::new();
    for (stem, path, size) in images.into_iter() {
        best.entry(stem)
            .and_modify(|(existing_path, existing_size)| {
                if size > 0 && (*existing_size == 0 || size < *existing_size) {
                    *existing_path = path.clone();
                    *existing_size = size;
                }
            })
            .or_insert((path, size));
    }

    let mut preferred: Vec<(u8, u64, PathBuf)> = Vec::new();
    for (stem, (path, size)) in best.iter() {
        out.by_stem.insert(stem.clone(), path.clone());
        if let Some(score) = preferred_score(stem) {
            preferred.push((score, *size, path.clone()));
        }
    }
    preferred.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    out.preferred_candidates = preferred.into_iter().map(|(_, _, p)| p).collect();

    out
}

fn read_cover_bytes(path: &Path) -> Option<Vec<u8>> {
    let size = std::fs::metadata(path).ok()?.len();
    if size == 0 || size > COVER_BYTES_LIMIT {
        return None;
    }
    std::fs::read(path).ok().and_then(|b| {
        if b.is_empty() || (b.len() as u64) > COVER_BYTES_LIMIT {
            None
        } else {
            Some(b)
        }
    })
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
        if let Some(v) = chosen.filter(|v| !v.data.is_empty()) {
            out.cover = Some(v.data.as_ref().to_vec());
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

pub(super) fn write_cover_bytes(cover_dir: &Path, track_id: i64, bytes: &[u8]) -> Result<()> {
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
