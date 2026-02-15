use crossbeam_channel::Receiver;
use tracing::debug;

use crate::types::{TrackDecodeInfo, TrackPlayability, TrackRef};
use stellatune_decode::{Decoder, TrackSpec, supports_path};
use stellatune_plugins::runtime::messages::WorkerControlMessage;
use stellatune_plugins::runtime::worker_endpoint::DecoderWorkerController;

mod gapless;
mod io;
mod runtime;

use self::gapless::{GaplessTrimSpec, GaplessTrimState};
use self::io::DecoderIoOwner;
use self::runtime::{
    SourceStreamLocator, build_builtin_track_info, decode_engine_track_token, ext_hint_from_path,
    has_decoder_candidates, runtime_has_source_catalog, try_open_decoder_for_local_path,
    try_open_decoder_for_source_stream,
};

pub(crate) enum EngineDecoder {
    Builtin {
        dec: Decoder,
        gapless: GaplessTrimState,
    },
    Plugin {
        controller: DecoderWorkerController,
        spec: TrackSpec,
        gapless: GaplessTrimState,
        control_rx: Receiver<WorkerControlMessage>,
        _io_owner: DecoderIoOwner,
    },
}

impl EngineDecoder {
    pub fn spec(&self) -> TrackSpec {
        match self {
            Self::Builtin { dec, .. } => dec.spec(),
            Self::Plugin { spec, .. } => *spec,
        }
    }

    pub fn seek_ms(&mut self, position_ms: u64) -> Result<(), String> {
        match self {
            Self::Builtin { dec, gapless } => {
                dec.seek_ms(position_ms).map_err(|e| e.to_string())?;
                gapless.reset_for_seek(position_ms);
                Ok(())
            },
            Self::Plugin {
                controller,
                gapless,
                ..
            } => {
                let dec = controller
                    .instance_mut()
                    .ok_or_else(|| "plugin decoder instance unavailable".to_string())?;
                dec.seek_ms(position_ms).map_err(|e| e.to_string())?;
                gapless.reset_for_seek(position_ms);
                Ok(())
            },
        }
    }

    fn raw_next_block(&mut self, frames: usize) -> Result<Option<Vec<f32>>, String> {
        match self {
            Self::Builtin { dec, .. } => dec.next_block(frames).map_err(|e| e.to_string()),
            Self::Plugin { controller, .. } => {
                let dec = controller
                    .instance_mut()
                    .ok_or_else(|| "plugin decoder instance unavailable".to_string())?;
                let (samples, _frames_read, eof) = dec
                    .read_interleaved_f32(frames as u32)
                    .map_err(|e| e.to_string())?;
                if samples.is_empty() {
                    if eof {
                        return Ok(None);
                    }
                    return Err("plugin decoder returned 0 frames without eof".to_string());
                }
                Ok(Some(samples))
            },
        }
    }

    fn gapless_state_mut(&mut self) -> &mut GaplessTrimState {
        match self {
            Self::Builtin { gapless, .. } => gapless,
            Self::Plugin { gapless, .. } => gapless,
        }
    }

    pub fn next_block(&mut self, frames: usize) -> Result<Option<Vec<f32>>, String> {
        let channels = self.spec().channels.max(1) as usize;
        let want_samples = frames.saturating_mul(channels).max(channels);

        loop {
            let need_more = {
                let gapless = self.gapless_state_mut();
                gapless.pending_output_len() < want_samples && !gapless.eof_reached()
            };
            if !need_more {
                break;
            }
            match self.raw_next_block(frames)? {
                Some(samples) => self.gapless_state_mut().push_decoded_samples(samples),
                None => self.gapless_state_mut().on_eof(),
            }
        }

        let gapless = self.gapless_state_mut();
        if gapless.pending_output_is_empty() {
            if gapless.eof_reached() {
                return Ok(None);
            }
            return Err("decoder produced no samples without eof".to_string());
        }

        Ok(Some(gapless.drain_pending(want_samples)))
    }

    pub fn has_pending_runtime_recreate(&mut self) -> bool {
        let Self::Plugin {
            controller,
            control_rx,
            ..
        } = self
        else {
            return false;
        };

        while let Ok(msg) = control_rx.try_recv() {
            controller.on_control_message(msg);
        }
        controller.has_pending_recreate() || controller.has_pending_destroy()
    }
}

pub(crate) fn assess_track_playability(track: &TrackRef) -> TrackPlayability {
    if track.source_id.trim().eq_ignore_ascii_case("local") {
        let path = track.locator.trim();
        if path.is_empty() {
            return TrackPlayability {
                track: track.clone(),
                playable: false,
                reason: Some("local_track_locator_empty".to_string()),
            };
        }
        if supports_path(path) {
            return TrackPlayability {
                track: track.clone(),
                playable: true,
                reason: None,
            };
        }
        let ext_hint = ext_hint_from_path(path);
        if has_decoder_candidates(&ext_hint, None, None) {
            return TrackPlayability {
                track: track.clone(),
                playable: true,
                reason: None,
            };
        }
        return TrackPlayability {
            track: track.clone(),
            playable: false,
            reason: Some("no_decoder_for_local_track".to_string()),
        };
    }

    let source = match serde_json::from_str::<SourceStreamLocator>(&track.locator) {
        Ok(v) => v,
        Err(_) => {
            return TrackPlayability {
                track: track.clone(),
                playable: false,
                reason: Some("invalid_source_track_locator".to_string()),
            };
        },
    };

    if !runtime_has_source_catalog(&source.plugin_id, &source.type_id) {
        return TrackPlayability {
            track: track.clone(),
            playable: false,
            reason: Some("source_catalog_unavailable".to_string()),
        };
    }

    if !has_decoder_candidates(
        source.ext_hint.trim(),
        source.decoder_plugin_id.as_deref(),
        source.decoder_type_id.as_deref(),
    ) {
        return TrackPlayability {
            track: track.clone(),
            playable: false,
            reason: Some("source_decoder_unavailable".to_string()),
        };
    }

    TrackPlayability {
        track: track.clone(),
        playable: true,
        reason: None,
    }
}

pub(crate) fn open_engine_decoder(
    track_token: &str,
) -> Result<(Box<EngineDecoder>, TrackDecodeInfo), String> {
    let track = decode_engine_track_token(track_token)?;

    if track.source_id.trim().eq_ignore_ascii_case("local") {
        let path = track.locator.trim();
        if path.is_empty() {
            return Err("local track locator is empty".to_string());
        }
        let ext_hint = ext_hint_from_path(path);

        if supports_path(path) {
            match Decoder::open(path) {
                Ok(d) => {
                    let spec = d.spec();
                    let info = build_builtin_track_info(spec);
                    let gapless = GaplessTrimState::new(
                        GaplessTrimSpec {
                            head_frames: d.encoder_delay_frames(),
                            tail_frames: d.encoder_padding_frames(),
                        },
                        spec.channels as usize,
                        spec.sample_rate,
                    );
                    debug!(path, "using built-in decoder for local track");
                    return Ok((Box::new(EngineDecoder::Builtin { dec: d, gapless }), info));
                },
                Err(e) => {
                    debug!("built-in decoder open failed, trying plugin decoders: {e}");
                },
            }
        }

        match try_open_decoder_for_local_path(path, &ext_hint) {
            Ok(Some((controller, info, gapless_spec, io_owner, control_rx))) => {
                return Ok((
                    Box::new(EngineDecoder::Plugin {
                        spec: TrackSpec {
                            sample_rate: info.sample_rate,
                            channels: info.channels,
                        },
                        controller,
                        gapless: GaplessTrimState::new(
                            gapless_spec,
                            info.channels as usize,
                            info.sample_rate,
                        ),
                        control_rx,
                        _io_owner: io_owner,
                    }),
                    info,
                ));
            },
            Ok(None) => {},
            Err(e) => {
                debug!("v2 local decoder open failed: {e}");
            },
        }

        let d = Decoder::open(path).map_err(|e| format!("failed to open decoder: {e}"))?;
        let spec = d.spec();
        let info = build_builtin_track_info(spec);
        let gapless = GaplessTrimState::new(
            GaplessTrimSpec {
                head_frames: d.encoder_delay_frames(),
                tail_frames: d.encoder_padding_frames(),
            },
            spec.channels as usize,
            spec.sample_rate,
        );
        return Ok((Box::new(EngineDecoder::Builtin { dec: d, gapless }), info));
    }

    let source = serde_json::from_str::<SourceStreamLocator>(&track.locator)
        .map_err(|e| format!("invalid source track locator json: {e}"))?;
    let ext_hint = source.ext_hint.trim().to_string();
    let path_hint = if source.path_hint.trim().is_empty() {
        track.stable_key()
    } else {
        source.path_hint.trim().to_string()
    };

    match try_open_decoder_for_source_stream(&source, &path_hint, &ext_hint) {
        Ok(Some((controller, info, gapless_spec, io_owner, control_rx))) => {
            return Ok((
                Box::new(EngineDecoder::Plugin {
                    spec: TrackSpec {
                        sample_rate: info.sample_rate,
                        channels: info.channels,
                    },
                    controller,
                    gapless: GaplessTrimState::new(
                        gapless_spec,
                        info.channels as usize,
                        info.sample_rate,
                    ),
                    control_rx,
                    _io_owner: io_owner,
                }),
                info,
            ));
        },
        Ok(None) => {},
        Err(e) => {
            debug!("v2 source decoder open failed: {e}");
        },
    }
    Err(format!(
        "failed to open v2 decoder on source stream `{path_hint}` (ext hint `{ext_hint}`)"
    ))
}
