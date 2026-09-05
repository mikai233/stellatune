use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use symphonia::core::common::Limit;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::{MetadataOptions, RawValue, StandardTag, StandardVisualKey};
use symphonia::core::units::{Time, TimeBase, Timestamp};
use symphonia::default::get_probe;
use tracing::debug;

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
    let meta_opts = MetadataOptions::default().limit_visual_bytes(Limit::Maximum(12 * 1024 * 1024));

    let mut format = match get_probe().probe(&hint, mss, FormatOptions::default(), meta_opts) {
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
        },
    };

    let mut out = ExtractedMetadata::default();

    // Metadata read during probing and from the container itself.
    {
        let mut m = format.metadata();
        if let Some(rev) = m.skip_to_latest() {
            apply_revision(rev, &mut out);
        }
    }

    // Duration estimate from codec params (fast, no decoding), with seek-based fallback.
    if let Some(track) = format.default_track(TrackType::Audio) {
        let track_id = track.id;
        let time_base = track.time_base;
        let n_frames = track.num_frames;
        let sample_rate = track
            .codec_params
            .as_ref()
            .and_then(|params| params.audio())
            .and_then(|params| params.sample_rate)
            .unwrap_or(0);
        let encoder_delay_frames = track.delay.unwrap_or(0);
        let encoder_padding_frames = track.padding.unwrap_or(0);
        out.duration_ms = duration_ms_from_track_params(time_base, n_frames);
        if out.duration_ms.is_none() {
            // TODO: Re-evaluate whether this seek-based duration fallback should be removed.
            out.duration_ms = estimate_duration_ms_by_seek(format.as_mut(), track_id, time_base);
        }
        out.duration_ms = trim_duration_ms_i64(
            out.duration_ms,
            sample_rate,
            encoder_delay_frames,
            encoder_padding_frames,
        );
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

pub(super) fn has_plugin_decoder_for_path(
    path: &Path,
    provider: &Option<std::sync::Arc<dyn crate::metadata_provider::MetadataProvider>>,
) -> bool {
    provider
        .as_ref()
        .is_some_and(|provider| provider.supports(path))
}

pub(super) fn extract_metadata_with_plugins(
    path: &Path,
    provider: &Option<std::sync::Arc<dyn crate::metadata_provider::MetadataProvider>>,
) -> Result<ExtractedMetadata> {
    if let Some(provider) = provider.as_ref().filter(|provider| provider.supports(path)) {
        let metadata = provider.inspect(path)?;
        return Ok(ExtractedMetadata {
            title: metadata.title,
            artist: metadata.artist,
            album: metadata.album,
            duration_ms: metadata.duration_ms,
            cover: None,
        });
    }
    extract_metadata(path)
}

fn duration_ms_from_track_params(
    time_base: Option<TimeBase>,
    n_frames: Option<u64>,
) -> Option<i64> {
    let tb = time_base?;
    let frames = n_frames?;
    Some(duration_ms_from_time_base(
        tb,
        Timestamp::new(frames.min(i64::MAX as u64) as i64),
    ))
}

fn duration_ms_from_time_base(tb: TimeBase, ts: Timestamp) -> i64 {
    tb.calc_time(ts)
        .map(|time| (time.as_secs_f64() * 1000.0).round() as i64)
        .unwrap_or(i64::MAX)
}

fn gapless_trim_delta_ms(
    sample_rate: u32,
    encoder_delay_frames: u32,
    encoder_padding_frames: u32,
) -> u64 {
    let sample_rate = sample_rate.max(1) as u128;
    let trimmed_frames =
        (encoder_delay_frames as u128).saturating_add(encoder_padding_frames as u128);
    if trimmed_frames == 0 {
        return 0;
    }

    trimmed_frames
        .saturating_mul(1000)
        .saturating_add(sample_rate / 2)
        .saturating_div(sample_rate)
        .min(u64::MAX as u128) as u64
}

fn trim_duration_ms_i64(
    duration_ms: Option<i64>,
    sample_rate: u32,
    encoder_delay_frames: u32,
    encoder_padding_frames: u32,
) -> Option<i64> {
    let duration_ms = duration_ms?;
    let delta_ms = gapless_trim_delta_ms(sample_rate, encoder_delay_frames, encoder_padding_frames);
    Some(duration_ms.saturating_sub(delta_ms.min(i64::MAX as u64) as i64))
}

fn estimate_duration_ms_by_seek(
    format: &mut dyn FormatReader,
    track_id: u32,
    time_base: Option<TimeBase>,
) -> Option<i64> {
    let tb = time_base?;
    let seeked = format
        .seek(
            SeekMode::Coarse,
            SeekTo::Time {
                time: Time::MAX,
                track_id: Some(track_id),
            },
        )
        .ok()?;
    let end_ts = seeked.actual_ts.max(seeked.required_ts);
    Some(duration_ms_from_time_base(tb, end_ts))
}

fn normalize_text_field(raw: &str) -> Option<String> {
    let text = raw.trim().to_string();
    if text.is_empty() { None } else { Some(text) }
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
    static DIR_IMAGE_INDEX: RefCell<HashMap<PathBuf, DirImageIndex>> =
        RefCell::new(HashMap::new());
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
    std::fs::read(path)
        .ok()
        .filter(|bytes| !bytes.is_empty() && (bytes.len() as u64) <= COVER_BYTES_LIMIT)
}

fn apply_revision(rev: &symphonia::core::meta::MetadataRevision, out: &mut ExtractedMetadata) {
    for tag in rev.media.tags.iter().chain(
        rev.per_track
            .iter()
            .flat_map(|track| track.metadata.tags.iter()),
    ) {
        match tag.std.as_ref() {
            Some(StandardTag::TrackTitle(value)) if out.title.is_none() => {
                out.title = normalize_text_field(value.as_str());
                continue;
            },
            Some(StandardTag::Artist(value)) if out.artist.is_none() => {
                out.artist = normalize_text_field(value.as_str());
                continue;
            },
            Some(StandardTag::Album(value)) if out.album.is_none() => {
                out.album = normalize_text_field(value.as_str());
                continue;
            },
            _ => {},
        }

        // Fallback for readers that don't assign a standard tag.
        if tag.std.is_none() {
            let key = tag.raw.key.trim().to_ascii_lowercase();
            match key.as_str() {
                "title" | "tracktitle" if out.title.is_none() => {
                    out.title = raw_value_to_string(&tag.raw.value);
                },
                "artist" if out.artist.is_none() => {
                    out.artist = raw_value_to_string(&tag.raw.value);
                },
                "album" if out.album.is_none() => {
                    out.album = raw_value_to_string(&tag.raw.value);
                },
                _ => {},
            }
        }
    }

    if out.cover.is_none() {
        let visuals = rev
            .media
            .visuals
            .iter()
            .chain(
                rev.per_track
                    .iter()
                    .flat_map(|track| track.metadata.visuals.iter()),
            )
            .collect::<Vec<_>>();
        let front = visuals
            .iter()
            .copied()
            .find(|v| v.usage == Some(StandardVisualKey::FrontCover));
        let any = visuals.first().copied();
        let chosen = front.or(any);
        if let Some(v) = chosen.filter(|v| !v.data.is_empty()) {
            out.cover = Some(v.data.as_ref().to_vec());
        }
    }
}

fn raw_value_to_string(v: &RawValue) -> Option<String> {
    let s = match v {
        RawValue::String(s) => s.to_string(),
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

#[cfg(test)]
mod tests {
    use super::trim_duration_ms_i64;

    #[test]
    fn trims_gapless_padding_from_i64_duration() {
        assert_eq!(trim_duration_ms_i64(Some(10), 1000, 3, 2), Some(5));
    }

    #[test]
    fn leaves_duration_unchanged_without_gapless_padding() {
        assert_eq!(trim_duration_ms_i64(Some(10), 44100, 0, 0), Some(10));
    }
}
