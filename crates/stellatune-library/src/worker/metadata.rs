use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::{Limit, MetadataOptions, StandardTagKey, StandardVisualKey, Value};
use symphonia::core::probe::Hint;
use symphonia::default::get_probe;

#[derive(Default)]
pub(super) struct ExtractedMetadata {
    pub(super) title: Option<String>,
    pub(super) artist: Option<String>,
    pub(super) album: Option<String>,
    pub(super) duration_ms: Option<i64>,
    pub(super) cover: Option<Vec<u8>>,
}

pub(super) fn extract_metadata(path: &Path) -> Result<ExtractedMetadata> {
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

    if out.cover.is_none() {
        out.cover = load_sidecar_cover(path);
    }

    Ok(out)
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

        if let Some(p) = idx.by_stem.get(&stem_key).cloned() {
            if let Some(bytes) = read_cover_bytes(&p) {
                return Some(bytes);
            }
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
