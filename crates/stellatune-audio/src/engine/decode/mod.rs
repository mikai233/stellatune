use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU8, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender};
use tracing::debug;

use crate::engine::control::{InternalDispatch, internal_eof_dispatch, internal_error_dispatch};
use crate::types::TrackDecodeInfo;
use stellatune_mixer::{ChannelLayout, ChannelMixer};

use crate::engine::messages::{DecodeCtrl, OutputSinkTx};

pub mod audio_path;
pub mod context;
pub mod controls;
pub mod decoder;
pub mod dsp;
pub mod resampler;
pub mod utils;

use self::audio_path::{handle_eof_and_flush, process_audio_no_resample, process_audio_resampled};
use self::context::DecodeContext;
use self::controls::{
    emit_position, handle_paused_controls, handle_playing_controls, perform_seek,
};
use self::decoder::{EngineDecoder, open_engine_decoder};
use self::dsp::ActiveDspNode;
use self::resampler::create_resampler_if_needed;
use self::utils::{core_lfe_to_mixer, skip_frames_by_decoding};

type DecodeSetupState = (
    Arc<Mutex<crate::ring_buffer::RingBufferProducer<f32>>>,
    u32,
    u16,
    Option<crate::engine::messages::PredecodedChunk>,
    i64,
    Arc<std::sync::atomic::AtomicBool>,
    i64,
    crate::types::LfeMode,
    Option<OutputSinkTx>,
    u32,
    bool,
    crate::types::ResampleQuality,
);

pub(crate) struct DecodeThreadArgs {
    pub(crate) path: String,
    pub(crate) internal_tx: Sender<InternalDispatch>,
    pub(crate) ctrl_rx: Receiver<DecodeCtrl>,
    pub(crate) setup_rx: Receiver<DecodeCtrl>,
    pub(crate) spec_tx: Sender<Result<TrackDecodeInfo, String>>,
    pub(crate) runtime_state: Arc<AtomicU8>,
}

pub(crate) fn decode_thread(args: DecodeThreadArgs) {
    let DecodeThreadArgs {
        path,
        internal_tx,
        ctrl_rx,
        setup_rx,
        spec_tx,
        runtime_state,
    } = args;

    let t_open = Instant::now();
    let (mut decoder, info): (Box<EngineDecoder>, TrackDecodeInfo) =
        match open_engine_decoder(&path) {
            Ok(v) => v,
            Err(e) => {
                let _ = spec_tx.send(Err(e));
                return;
            }
        };
    debug!("decoder open took {}ms", t_open.elapsed().as_millis());

    let spec = decoder.spec();
    let _ = spec_tx.send(Ok(info));

    let (
        producer,
        target_sample_rate,
        target_channels,
        predecoded,
        start_at_ms,
        output_enabled,
        buffer_prefill_cap_ms,
        initial_lfe_mode,
        mut output_sink_tx,
        mut output_sink_chunk_frames,
        output_sink_only,
        resample_quality,
    ): DecodeSetupState = loop {
        crossbeam_channel::select! {
            recv(setup_rx) -> msg => {
                let Ok(ctrl) = msg else { return };
                if let DecodeCtrl::Setup {
                    producer: ring_buffer_producer,
                    target_sample_rate,
                    target_channels,
                    predecoded,
                    start_at_ms,
                    output_enabled,
                    buffer_prefill_cap_ms,
                    lfe_mode: initial_lfe_mode,
                    output_sink_tx,
                    output_sink_chunk_frames,
                    output_sink_only,
                    resample_quality,
                } = ctrl {
                    break (
                        ring_buffer_producer,
                        target_sample_rate,
                        target_channels,
                        predecoded,
                        start_at_ms,
                        output_enabled,
                        buffer_prefill_cap_ms,
                        initial_lfe_mode,
                        output_sink_tx,
                        output_sink_chunk_frames,
                        output_sink_only,
                        resample_quality,
                    );
                }
            }
            recv(ctrl_rx) -> msg => {
                let Ok(msg) = msg else { return };
                if matches!(msg, DecodeCtrl::Stop) {
                    return;
                }
            }
        }
    };

    let in_channels = spec.channels as usize;
    let out_channels = target_channels as usize;

    let mut base_ms = start_at_ms.max(0);
    let expected_predecoded_start = base_ms as u64;

    let t_resampler = Instant::now();
    let mut resampler = match create_resampler_if_needed(
        spec.sample_rate,
        target_sample_rate,
        out_channels,
        resample_quality,
    ) {
        Ok(r) => r,
        Err(e) => {
            let _ = internal_tx.send(internal_error_dispatch(e));
            return;
        }
    };
    debug!(
        "resampler init: {} ({}ms)",
        if resampler.is_some() {
            "enabled"
        } else {
            "bypass"
        },
        t_resampler.elapsed().as_millis()
    );

    let mut playing = false;
    let mut frames_written: u64 = 0;
    let mut last_emit = Instant::now();
    let mut decode_pending: Vec<f32> = Vec::new();
    let mut out_pending: Vec<f32> = Vec::new();
    let mut used_predecoded = false;
    if let Some(chunk) = predecoded {
        if chunk.start_at_ms == expected_predecoded_start
            && chunk.sample_rate == spec.sample_rate
            && chunk.channels as usize == in_channels
        {
            decode_pending.extend_from_slice(&chunk.samples);
            used_predecoded = true;
        } else {
            debug!(
                "predecoded chunk mismatch: chunk_start={}ms expected={}ms chunk={}Hz {}ch decoder={}Hz {}ch",
                chunk.start_at_ms,
                expected_predecoded_start,
                chunk.sample_rate,
                chunk.channels,
                spec.sample_rate,
                in_channels
            );
        }
    }
    if base_ms > 0 && !used_predecoded {
        let t_skip = Instant::now();
        let frames_to_skip = ((base_ms as i128 * spec.sample_rate as i128) / 1000) as u64;
        if !skip_frames_by_decoding(&mut decoder, frames_to_skip) {
            let _ = internal_tx.send(internal_eof_dispatch());
            return;
        }
        debug!(
            "fast-forward by decoding/discarding: start_at_ms={} took {}ms",
            base_ms,
            t_skip.elapsed().as_millis()
        );
    }

    let mut dsp_chain: Vec<ActiveDspNode> = Vec::new();

    let mut lfe_mode = core_lfe_to_mixer(initial_lfe_mode);
    let mut channel_mixer = ChannelMixer::new(
        ChannelLayout::from_count(in_channels as u16),
        ChannelLayout::from_count(out_channels as u16),
        lfe_mode,
    );

    let mut pending_seek: Option<i64> = None;

    'main: loop {
        if playing && !output_enabled.load(Ordering::Acquire) {
            let buffered_frames = producer
                .lock()
                .map(|p| (p.len() / out_channels) as u64)
                .unwrap_or(0);
            let buffered_ms = ((buffered_frames * 1000) / target_sample_rate.max(1) as u64) as i64;
            if buffered_ms >= buffer_prefill_cap_ms {
                thread::sleep(Duration::from_millis(5));
                continue;
            }
        }

        let mut ctx = DecodeContext {
            path: &path,
            playing: &mut playing,
            last_emit: &mut last_emit,
            dsp_chain: &mut dsp_chain,
            decoder: &mut decoder,
            resampler: &mut resampler,
            producer: &producer,
            decode_pending: &mut decode_pending,
            out_pending: &mut out_pending,
            frames_written: &mut frames_written,
            base_ms: &mut base_ms,
            lfe_mode: &mut lfe_mode,
            channel_mixer: &mut channel_mixer,
            pending_seek: &mut pending_seek,
            in_channels,
            out_channels,
            spec_sample_rate: spec.sample_rate,
            target_sample_rate,
            output_enabled: &output_enabled,
            output_sink_tx: &mut output_sink_tx,
            output_sink_chunk_frames: &mut output_sink_chunk_frames,
            output_sink_only,
            resample_quality,
            ctrl_rx: &ctrl_rx,
            internal_tx: &internal_tx,
        };

        if !*ctx.playing {
            if handle_paused_controls(&mut ctx, &runtime_state) {
                break;
            }
            continue;
        }

        if handle_playing_controls(&mut ctx, &runtime_state) {
            return;
        }

        if !*ctx.playing {
            continue;
        }

        if !ctx.decode_pending.is_empty() {
            if ctx.resampler.is_none() {
                if process_audio_no_resample(&mut ctx) {
                    return;
                }
            } else {
                match process_audio_resampled(&mut ctx) {
                    Ok(true) => return,
                    Ok(false) => {}
                    Err(e) => {
                        let _ = ctx.internal_tx.send(internal_error_dispatch(e));
                        return;
                    }
                }
            }

            if let Some(seek_ms) = ctx.pending_seek.take() {
                if let Err(e) = perform_seek(seek_ms, &mut ctx) {
                    let _ = ctx.internal_tx.send(internal_error_dispatch(e));
                    *ctx.playing = false;
                }
                continue 'main;
            }
        }

        emit_position(&mut ctx);

        match ctx.decoder.next_block(4096) {
            Ok(Some(samples)) => {
                ctx.decode_pending.extend_from_slice(&samples);
                if ctx.resampler.is_none() {
                    if process_audio_no_resample(&mut ctx) {
                        return;
                    }
                } else {
                    match process_audio_resampled(&mut ctx) {
                        Ok(true) => return,
                        Ok(false) => {}
                        Err(e) => {
                            let _ = ctx.internal_tx.send(internal_error_dispatch(e));
                            return;
                        }
                    }
                }

                if let Some(seek_ms) = ctx.pending_seek.take() {
                    if let Err(e) = perform_seek(seek_ms, &mut ctx) {
                        let _ = ctx.internal_tx.send(internal_error_dispatch(e));
                        *ctx.playing = false;
                    }
                    continue 'main;
                }
            }
            Ok(None) => {
                if handle_eof_and_flush(&mut ctx) {
                    return;
                }
                if let Some(seek_ms) = ctx.pending_seek.take() {
                    if let Err(e) = perform_seek(seek_ms, &mut ctx) {
                        let _ = ctx.internal_tx.send(internal_error_dispatch(e));
                        *ctx.playing = false;
                    }
                    continue 'main;
                }
                let _ = ctx.internal_tx.send(internal_eof_dispatch());
                break;
            }
            Err(e) => {
                let _ = ctx.internal_tx.send(internal_error_dispatch(e));
                break;
            }
        }
    }
}
