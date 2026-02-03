use std::path::Path;

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
