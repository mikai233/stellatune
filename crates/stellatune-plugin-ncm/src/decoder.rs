use std::io::{Seek, SeekFrom};

use stellatune_plugin_sdk::StLogLevel;
use stellatune_plugin_sdk::{Decoder, DecoderDescriptor, DecoderInfo, DecoderOpenArgs};
use symphonia::core::audio::{SampleBuffer, SignalSpec};
use symphonia::core::codecs::{Decoder as SymphoniaDecoder, DecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::Time;

use crate::flac_offset::find_flac_streaminfo_start;
use crate::io::NcmMediaSource;
use crate::tags::{Tags, apply_revision, build_metadata_json};

const NCM_MAGIC: &[u8; 8] = b"CTENFDAM";

pub struct NcmDecoder {
    backend: NcmBackend,
}

enum NcmBackend {
    Symphonia(SymphoniaBackend),
}

struct SymphoniaBackend {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn SymphoniaDecoder>,
    track_id: u32,
    in_channels: usize,
    spec: stellatune_plugin_sdk::StAudioSpec,
    sample_buf: Option<SampleBuffer<f32>>,
    pending: Vec<f32>,
    metadata_json: Option<String>,
    duration_ms: Option<u64>,
    seekable: bool,
}

impl NcmDecoder {
    fn open_from_io(io: stellatune_plugin_sdk::HostIo) -> Result<Self, String> {
        let mut io_copy = io;
        let file_size = io_copy.size().map_err(|e| e.to_string())?;

        let mut ncm =
            ncmdump::Ncmdump::from_reader(io).map_err(|e| format!("ncmdump parse failed: {e}"))?;

        let audio_start_abs = io_copy
            .stream_position()
            .map_err(|e| format!("tell failed: {e}"))?;
        let len = file_size.saturating_sub(audio_start_abs);

        let info = ncm
            .get_info()
            .map_err(|e| format!("ncmdump get_info failed: {e}"))?;
        let cover = ncm.get_image().ok();
        stellatune_plugin_sdk::host_log!(
            StLogLevel::Debug,
            "ncm: container info title={:?} fmt={:?} duration_ms={} cover={}",
            info.name.trim(),
            info.format.trim(),
            info.duration,
            cover.as_ref().map(|b| !b.is_empty()).unwrap_or(false)
        );

        ncm.seek(SeekFrom::Start(0))
            .map_err(|e| format!("ncmdump seek to audio start failed: {e}"))?;

        let hint_ext = info.format.trim().to_string();
        let duration_ms_container = Some(info.duration).filter(|v| *v > 0);

        let mut tags = Tags {
            title: Some(info.name.trim().to_string()).filter(|s| !s.is_empty()),
            album: Some(info.album.trim().to_string()).filter(|s| !s.is_empty()),
            artist: if info.artist.is_empty() {
                None
            } else {
                Some(
                    info.artist
                        .iter()
                        .map(|(name, _id)| name.trim())
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                        .join(" / "),
                )
                .filter(|s| !s.is_empty())
            },
            cover: cover.filter(|b| !b.is_empty()),
        };

        let mut start_offset: u64 = 0;
        if hint_ext.eq_ignore_ascii_case("flac") {
            start_offset = find_flac_streaminfo_start(&mut ncm)?;
            if start_offset != 0 {
                stellatune_plugin_sdk::host_log!(
                    StLogLevel::Debug,
                    "ncm: flac payload has leading junk, skipping {} bytes",
                    start_offset
                );
                ncm.seek(SeekFrom::Start(start_offset))
                    .map_err(|e| format!("ncmdump seek to flac start failed: {e}"))?;
            }
        }

        let mut hint = Hint::new();
        if !hint_ext.is_empty() {
            hint.with_extension(&hint_ext);
        }

        let len = len.saturating_sub(start_offset);
        let src = NcmMediaSource::new(ncm, start_offset, len);
        let mss = MediaSourceStream::new(Box::new(src), MediaSourceStreamOptions::default());

        let meta_opts = MetadataOptions {
            limit_visual_bytes: symphonia::core::meta::Limit::Maximum(12 * 1024 * 1024),
            ..Default::default()
        };

        let mut probed = symphonia::default::get_probe()
            .format(&hint, mss, &FormatOptions::default(), &meta_opts)
            .map_err(|e| format!("symphonia probe failed: {e}"))?;

        let mut format = probed.format;
        let track = format
            .default_track()
            .ok_or_else(|| "missing audio track".to_string())?;
        let track_id = track.id;
        let params = track.codec_params.clone();

        let sample_rate = params
            .sample_rate
            .ok_or_else(|| "missing sample rate".to_string())?;
        let channels = params.channels.as_ref().map(|c| c.count()).unwrap_or(0);
        let in_channels = channels as usize;
        if in_channels == 0 {
            return Err("missing channels".to_string());
        }
        let out_channels: u16 = if in_channels == 1 { 1 } else { 2 };

        let decoder = symphonia::default::get_codecs()
            .make(&params, &DecoderOptions::default())
            .map_err(|e| format!("decoder init failed: {e}"))?;

        let duration_ms = duration_ms_container.or_else(|| {
            params.time_base.and_then(|tb| {
                params.n_frames.map(|n| {
                    let t = tb.calc_time(n);
                    let ms = (t.seconds as f64 * 1000.0) + (t.frac * 1000.0);
                    ms.round() as u64
                })
            })
        });

        if let Some(mut m) = probed.metadata.get()
            && let Some(rev) = m.skip_to_latest()
        {
            apply_revision(rev, &mut tags);
        }
        {
            let mut m = format.metadata();
            if let Some(rev) = m.skip_to_latest() {
                apply_revision(rev, &mut tags);
            }
        }

        let metadata_json = build_metadata_json(tags, duration_ms);
        stellatune_plugin_sdk::host_log!(
            StLogLevel::Info,
            "ncm: opened sr={} ch={} hint_ext={:?} duration_ms={:?}",
            sample_rate,
            channels,
            hint_ext,
            duration_ms
        );

        Ok(Self {
            backend: NcmBackend::Symphonia(SymphoniaBackend {
                format,
                decoder,
                track_id,
                in_channels,
                spec: stellatune_plugin_sdk::StAudioSpec {
                    sample_rate,
                    channels: out_channels,
                    reserved: 0,
                },
                sample_buf: None,
                pending: Vec::new(),
                metadata_json,
                duration_ms,
                seekable: true,
            }),
        })
    }
}

impl SymphoniaBackend {
    fn info(&self) -> DecoderInfo {
        DecoderInfo {
            spec: self.spec,
            duration_ms: self.duration_ms,
            seekable: self.seekable,
        }
    }

    fn metadata_json(&self) -> Option<String> {
        self.metadata_json.clone()
    }

    fn seek_ms(&mut self, position_ms: u64) -> Result<(), String> {
        if !self.seekable {
            return Err("seek not supported".to_string());
        }
        let secs = position_ms / 1000;
        let frac = (position_ms % 1000) as f64 / 1000.0;
        let time = Time::new(secs, frac);
        let _ = self
            .format
            .seek(
                SeekMode::Accurate,
                SeekTo::Time {
                    time,
                    track_id: Some(self.track_id),
                },
            )
            .map_err(|e| e.to_string())?;
        self.decoder.reset();
        self.pending.clear();
        Ok(())
    }

    fn read_interleaved_f32(
        &mut self,
        frames: u32,
        out_interleaved: &mut [f32],
    ) -> Result<(u32, bool), String> {
        let want = (frames as usize).saturating_mul(self.spec.channels.max(1) as usize);
        if out_interleaved.len() < want {
            return Err("output buffer too small".to_string());
        }

        while self.pending.len() < want {
            match self.format.next_packet() {
                Ok(packet) => {
                    if packet.track_id() != self.track_id {
                        continue;
                    }
                    match self.decoder.decode(&packet) {
                        Ok(audio_buf) => {
                            let spec =
                                SignalSpec::new(audio_buf.spec().rate, audio_buf.spec().channels);
                            let duration = audio_buf.capacity() as u64;

                            let needs_realloc = match self.sample_buf.as_ref() {
                                None => true,
                                Some(buf) => buf.capacity() < audio_buf.capacity(),
                            };
                            if needs_realloc {
                                self.sample_buf = Some(SampleBuffer::<f32>::new(duration, spec));
                            }

                            let sb = self.sample_buf.as_mut().expect("just initialized");
                            sb.copy_interleaved_ref(audio_buf);

                            let in_ch = self.in_channels.max(1);
                            let out_ch = self.spec.channels.max(1) as usize;
                            let samples = sb.samples();
                            let frames = samples.len() / in_ch;

                            if in_ch == out_ch {
                                self.pending.extend_from_slice(samples);
                            } else if out_ch == 1 {
                                for i in 0..frames {
                                    let l = samples[i * in_ch];
                                    let r = samples[i * in_ch + 1];
                                    self.pending.push((l + r) * 0.5);
                                }
                            } else {
                                let extra = (in_ch.saturating_sub(2)) as f32;
                                let norm = 1.0 / (1.0 + 0.5 * extra).max(1.0);
                                for i in 0..frames {
                                    let base = i * in_ch;
                                    let mut l = samples[base];
                                    let mut r = samples[base + 1];
                                    for ch in 2..in_ch {
                                        let v = samples[base + ch];
                                        l += v * 0.5;
                                        r += v * 0.5;
                                    }
                                    self.pending.push((l * norm).clamp(-1.0, 1.0));
                                    self.pending.push((r * norm).clamp(-1.0, 1.0));
                                }
                            }
                        }
                        Err(SymphoniaError::DecodeError(_)) => continue,
                        Err(SymphoniaError::ResetRequired) => {
                            self.decoder.reset();
                            continue;
                        }
                        Err(e) => return Err(e.to_string()),
                    }
                }
                Err(SymphoniaError::IoError(e))
                    if e.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    break;
                }
                Err(e) => return Err(e.to_string()),
            }
        }

        if self.pending.is_empty() {
            return Ok((0, true));
        }

        let take = want.min(self.pending.len());
        out_interleaved[..take].copy_from_slice(&self.pending[..take]);
        self.pending.drain(..take);
        Ok(((take / self.spec.channels.max(1) as usize) as u32, false))
    }
}

impl Decoder for NcmDecoder {
    fn info(&self) -> DecoderInfo {
        match &self.backend {
            NcmBackend::Symphonia(b) => b.info(),
        }
    }

    fn metadata_json(&self) -> Option<String> {
        match &self.backend {
            NcmBackend::Symphonia(b) => b.metadata_json(),
        }
    }

    fn seek_ms(&mut self, position_ms: u64) -> Result<(), String> {
        match &mut self.backend {
            NcmBackend::Symphonia(b) => b.seek_ms(position_ms),
        }
    }

    fn read_interleaved_f32(
        &mut self,
        frames: u32,
        out_interleaved: &mut [f32],
    ) -> Result<(u32, bool), String> {
        match &mut self.backend {
            NcmBackend::Symphonia(b) => b.read_interleaved_f32(frames, out_interleaved),
        }
    }
}

impl DecoderDescriptor for NcmDecoder {
    const TYPE_ID: &'static str = "ncm";
    const SUPPORTS_SEEK: bool = true;

    fn probe(path_ext: &str, header: &[u8]) -> u8 {
        let ext_ok = path_ext.eq_ignore_ascii_case("ncm");
        let magic_ok = header.len() >= NCM_MAGIC.len() && &header[..NCM_MAGIC.len()] == NCM_MAGIC;
        if ext_ok && magic_ok {
            100
        } else if ext_ok {
            60
        } else if magic_ok {
            80
        } else {
            0
        }
    }

    fn open(args: DecoderOpenArgs<'_>) -> Result<Self, String> {
        let io = args.io;
        stellatune_plugin_sdk::host_log!(
            StLogLevel::Info,
            "ncm: open path={:?} ext={:?} seekable={}",
            args.path,
            args.ext,
            io.is_seekable()
        );
        if !io.is_seekable() {
            return Err("ncm: io must be seekable".to_string());
        }
        Self::open_from_io(io)
    }
}
