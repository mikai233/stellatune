use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::builtin_decoder::BuiltinDecoder;
use stellatune_audio_core::pipeline::context::{GaplessTrimSpec, StreamSpec};

pub struct PlaylistDecoder {
    segment_paths: Vec<PathBuf>,
    current_index: usize,
    active_decoder: Option<BuiltinDecoder>,
    spec: StreamSpec,
    duration_ms_hint: Option<u64>,
}

impl PlaylistDecoder {
    pub fn open(path: &str) -> Result<Self, String> {
        let path = Path::new(path);
        let base_dir = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();

        let mut file = File::open(path).map_err(|e| format!("failed to open playlist: {e}"))?;
        let mut content = Vec::new();
        file.read_to_end(&mut content)
            .map_err(|e| format!("failed to read playlist: {e}"))?;

        let mut segment_paths = Vec::new();

        let mut duration_ms_hint = 0u64;
        let mut has_durations = false;

        // Try parsing as HLS first via m3u8-rs
        match m3u8_rs::parse_playlist_res(&content) {
            Ok(m3u8_rs::Playlist::MasterPlaylist(_)) => {
                return Err(
                    "Master playlists (multi-variant HLS) are not supported yet".to_string()
                );
            },
            Ok(m3u8_rs::Playlist::MediaPlaylist(media)) => {
                for segment in media.segments {
                    let segment_path = base_dir.join(segment.uri);
                    segment_paths.push(segment_path);
                    duration_ms_hint += (segment.duration * 1000.0) as u64;
                    has_durations = true;
                }
            },
            Err(_) => {
                // Fallback to simple M3U parsing (one path per line, skip comments)
                let text = String::from_utf8_lossy(&content);
                for line in text.lines() {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with('#') {
                        continue;
                    }
                    segment_paths.push(base_dir.join(line));
                }
            },
        }

        if segment_paths.is_empty() {
            return Err(format!(
                "playlist at {:?} contains no valid segments",
                base_dir
            ));
        }

        // Open the first segment to get the initial spec
        let decoder = BuiltinDecoder::open(segment_paths[0].to_str().ok_or("invalid path")?)?;
        let spec = decoder.spec();
        let duration_ms_hint = if has_durations {
            Some(duration_ms_hint)
        } else {
            None
        };

        Ok(Self {
            segment_paths,
            current_index: 0,
            active_decoder: Some(decoder),
            spec,
            duration_ms_hint,
        })
    }

    pub fn spec(&self) -> StreamSpec {
        self.spec
    }

    pub fn duration_ms_hint(&self) -> Option<u64> {
        self.duration_ms_hint
    }

    pub fn gapless_trim_spec(&self) -> Option<GaplessTrimSpec> {
        // Gapless across segments is complex; delegate to active decoder for internal segment trimming
        self.active_decoder
            .as_ref()
            .and_then(|d| d.gapless_trim_spec())
    }

    pub fn seek_ms(&mut self, _position_ms: u64) -> Result<(), String> {
        // Seeking in a multi-segment playlist requires indexing segment durations
        // and mapping the position to a specific segment and internal offset.
        Err("Seek is not currently supported for playlist decoders".to_string())
    }

    pub fn next_block(&mut self, frames: usize) -> Result<Option<Vec<f32>>, String> {
        loop {
            let Some(decoder) = self.active_decoder.as_mut() else {
                return Ok(None);
            };

            match decoder.next_block(frames)? {
                Some(block) => return Ok(Some(block)),
                None => {
                    // Current segment EOF, try next
                    self.current_index += 1;
                    if self.current_index >= self.segment_paths.len() {
                        self.active_decoder = None;
                        return Ok(None);
                    }

                    let next_path = &self.segment_paths[self.current_index];
                    let next_path_str = next_path.to_str().ok_or("invalid path")?;

                    let next_decoder = BuiltinDecoder::open(next_path_str)?;
                    if next_decoder.spec() != self.spec {
                        return Err(format!(
                            "spec mismatch at segment {}: expected {:?}, got {:?}",
                            self.current_index,
                            self.spec,
                            next_decoder.spec()
                        ));
                    }
                    self.active_decoder = Some(next_decoder);
                },
            }
        }
    }
}
