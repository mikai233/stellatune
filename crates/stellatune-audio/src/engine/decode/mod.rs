use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU8, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender};
use tracing::debug;

use stellatune_core::TrackDecodeInfo;
use stellatune_mixer::{ChannelLayout, ChannelMixer};

use crate::engine::config::RESAMPLE_CHUNK_FRAMES;
use crate::engine::event_hub::EventHub;
use crate::engine::messages::{
    DecodeCtrl, DecodeWorkerState, InternalMsg, OutputSinkTx, RuntimeDspChainEntry,
};

pub mod context;
pub mod decoder;
pub mod dsp;
pub mod resampler;
pub mod utils;

use self::context::DecodeContext;
use self::decoder::{EngineDecoder, open_engine_decoder};
use self::dsp::{ActiveDspNode, DspStage, apply_dsp_stage, apply_or_recreate_dsp_chain};
use self::resampler::{create_resampler_if_needed, resample_interleaved_chunk};
use self::utils::{
    adapt_channels_interleaved, core_lfe_to_mixer, refresh_decoder, skip_frames_by_decoding,
    write_pending,
};

type DecodeSetupState = (
    Arc<Mutex<crate::ring_buffer::RingBufferProducer<f32>>>,
    u32,
    u16,
    Option<crate::engine::messages::PredecodedChunk>,
    i64,
    Arc<std::sync::atomic::AtomicBool>,
    i64,
    stellatune_core::LfeMode,
    Option<OutputSinkTx>,
    u32,
    bool,
);

pub(crate) struct DecodeThreadArgs {
    pub(crate) path: String,
    pub(crate) events: Arc<EventHub>,
    pub(crate) internal_tx: Sender<InternalMsg>,
    pub(crate) preopened: Option<(Box<EngineDecoder>, TrackDecodeInfo)>,
    pub(crate) ctrl_rx: Receiver<DecodeCtrl>,
    pub(crate) setup_rx: Receiver<DecodeCtrl>,
    pub(crate) spec_tx: Sender<Result<TrackDecodeInfo, String>>,
    pub(crate) runtime_state: Arc<AtomicU8>,
}

pub(crate) fn decode_thread(args: DecodeThreadArgs) {
    let DecodeThreadArgs {
        path,
        events,
        internal_tx,
        preopened,
        ctrl_rx,
        setup_rx,
        spec_tx,
        runtime_state,
    } = args;

    let (mut decoder, info): (Box<EngineDecoder>, TrackDecodeInfo) =
        if let Some((decoder, info)) = preopened {
            debug!("decoder open reused preloaded instance");
            (decoder, info)
        } else {
            let t_open = Instant::now();
            let (decoder, info) = match open_engine_decoder(&path) {
                Ok(v) => v,
                Err(e) => {
                    let _ = spec_tx.send(Err(e));
                    return;
                }
            };
            debug!("decoder open took {}ms", t_open.elapsed().as_millis());
            (decoder, info)
        };

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
    let mut resampler =
        match create_resampler_if_needed(spec.sample_rate, target_sample_rate, out_channels) {
            Ok(r) => r,
            Err(e) => {
                let _ = internal_tx.send(InternalMsg::Error(e));
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
            let _ = internal_tx.send(InternalMsg::Eof);
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
            events: &events,
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
                        let _ = ctx.internal_tx.send(InternalMsg::Error(e));
                        return;
                    }
                }
            }

            if let Some(seek_ms) = ctx.pending_seek.take() {
                if let Err(e) = perform_seek(seek_ms, &mut ctx) {
                    let _ = ctx.internal_tx.send(InternalMsg::Error(e));
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
                            let _ = ctx.internal_tx.send(InternalMsg::Error(e));
                            return;
                        }
                    }
                }

                if let Some(seek_ms) = ctx.pending_seek.take() {
                    if let Err(e) = perform_seek(seek_ms, &mut ctx) {
                        let _ = ctx.internal_tx.send(InternalMsg::Error(e));
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
                        let _ = ctx.internal_tx.send(InternalMsg::Error(e));
                        *ctx.playing = false;
                    }
                    continue 'main;
                }
                let _ = ctx.internal_tx.send(InternalMsg::Eof);
                break;
            }
            Err(e) => {
                let _ = ctx.internal_tx.send(InternalMsg::Error(e));
                break;
            }
        }
    }
}

fn set_decode_worker_state(runtime_state: &Arc<AtomicU8>, next: DecodeWorkerState, reason: &str) {
    let prev = runtime_state.swap(next as u8, Ordering::Relaxed);
    if prev == next as u8 {
        return;
    }
    let prev = DecodeWorkerState::from_u8(prev);
    debug!(from = ?prev, to = ?next, reason, "decode worker state");
}

fn perform_seek(target_ms: i64, ctx: &mut DecodeContext) -> Result<(), String> {
    let target_ms = target_ms.max(0);
    ctx.output_enabled.store(false, Ordering::Release);
    if let Ok(mut producer) = ctx.producer.lock() {
        producer.clear();
    }
    ctx.decode_pending.clear();
    ctx.out_pending.clear();
    *ctx.frames_written = 0;
    *ctx.base_ms = target_ms;

    ctx.decoder.seek_ms(target_ms as u64)?;
    *ctx.resampler = create_resampler_if_needed(
        ctx.spec_sample_rate,
        ctx.target_sample_rate,
        ctx.out_channels,
    )?;
    *ctx.last_emit = Instant::now();
    Ok(())
}

fn handle_paused_controls(ctx: &mut DecodeContext, runtime_state: &Arc<AtomicU8>) -> bool {
    match ctx.ctrl_rx.recv() {
        Ok(DecodeCtrl::Play) => {
            *ctx.playing = true;
            *ctx.last_emit = Instant::now();
            set_decode_worker_state(runtime_state, DecodeWorkerState::Playing, "play");
        }
        Ok(DecodeCtrl::Pause) => {
            set_decode_worker_state(runtime_state, DecodeWorkerState::Paused, "pause");
        }
        Ok(DecodeCtrl::SetDspChain { chain }) => {
            if let Err(e) = sync_dsp_chain(ctx, chain) {
                let _ = ctx.internal_tx.send(InternalMsg::Error(e));
            }
        }
        Ok(DecodeCtrl::RefreshDecoder) => {
            if let Err(e) = refresh_decoder(ctx) {
                debug!("decoder refresh failed: {e}");
            }
        }
        Ok(DecodeCtrl::SeekMs { position_ms }) => {
            if let Err(e) = perform_seek(position_ms, ctx) {
                let _ = ctx.internal_tx.send(InternalMsg::Error(e));
            }
        }
        Ok(DecodeCtrl::SetLfeMode { mode }) => {
            *ctx.lfe_mode = core_lfe_to_mixer(mode);
            *ctx.channel_mixer = ChannelMixer::new(
                ChannelLayout::from_count(ctx.in_channels as u16),
                ChannelLayout::from_count(ctx.out_channels as u16),
                *ctx.lfe_mode,
            );
        }
        Ok(DecodeCtrl::SetOutputSinkTx {
            tx,
            output_sink_chunk_frames,
        }) => {
            *ctx.output_sink_tx = tx;
            *ctx.output_sink_chunk_frames = output_sink_chunk_frames;
        }
        Ok(DecodeCtrl::Stop) | Err(_) => {
            set_decode_worker_state(runtime_state, DecodeWorkerState::Idle, "stop");
            return true;
        }
        _ => {}
    }
    false
}

fn handle_playing_controls(ctx: &mut DecodeContext, runtime_state: &Arc<AtomicU8>) -> bool {
    while let Ok(ctrl) = ctx.ctrl_rx.try_recv() {
        match ctrl {
            DecodeCtrl::SetLfeMode { mode } => {
                *ctx.lfe_mode = core_lfe_to_mixer(mode);
                *ctx.channel_mixer = ChannelMixer::new(
                    ChannelLayout::from_count(ctx.in_channels as u16),
                    ChannelLayout::from_count(ctx.out_channels as u16),
                    *ctx.lfe_mode,
                );
            }
            DecodeCtrl::Pause => {
                *ctx.playing = false;
                set_decode_worker_state(runtime_state, DecodeWorkerState::Paused, "pause");
                return false;
            }
            DecodeCtrl::SeekMs { position_ms } => {
                if let Err(e) = perform_seek(position_ms, ctx) {
                    let _ = ctx.internal_tx.send(InternalMsg::Error(e));
                    *ctx.playing = false;
                }
                return false;
            }
            DecodeCtrl::SetDspChain { chain } => {
                if let Err(e) = sync_dsp_chain(ctx, chain) {
                    let _ = ctx.internal_tx.send(InternalMsg::Error(e));
                }
            }
            DecodeCtrl::RefreshDecoder => {
                if let Err(e) = refresh_decoder(ctx) {
                    debug!("decoder refresh failed: {e}");
                }
            }
            DecodeCtrl::SetOutputSinkTx {
                tx,
                output_sink_chunk_frames,
            } => {
                *ctx.output_sink_tx = tx;
                *ctx.output_sink_chunk_frames = output_sink_chunk_frames;
            }
            DecodeCtrl::Stop => {
                set_decode_worker_state(runtime_state, DecodeWorkerState::Idle, "stop");
                return true;
            }
            _ => {}
        }
    }
    false
}

fn emit_position(ctx: &mut DecodeContext) {
    if ctx.last_emit.elapsed() >= Duration::from_millis(200) {
        let buffered_frames = ctx
            .producer
            .lock()
            .map(|p| (p.len() / ctx.out_channels) as u64)
            .unwrap_or(0);
        let played_frames = ctx.frames_written.saturating_sub(buffered_frames);
        let ms = ctx.base_ms.saturating_add(
            ((played_frames.saturating_mul(1000)) / ctx.target_sample_rate as u64) as i64,
        );
        ctx.events.emit(stellatune_core::Event::Position { ms });
        let _ = ctx.internal_tx.try_send(InternalMsg::Position(ms));
        *ctx.last_emit = Instant::now();
    }
}

fn process_audio_no_resample(ctx: &mut DecodeContext) -> bool {
    apply_dsp_stage(
        ctx.dsp_chain,
        DspStage::PreMix,
        ctx.decode_pending,
        ctx.in_channels,
    );

    let mut chunk = if ctx.in_channels == ctx.out_channels {
        std::mem::take(ctx.decode_pending)
    } else {
        let v = adapt_channels_interleaved(
            ctx.decode_pending,
            ctx.in_channels,
            ctx.out_channels,
            ctx.channel_mixer,
        );
        ctx.decode_pending.clear();
        v
    };

    apply_dsp_stage(
        ctx.dsp_chain,
        DspStage::PostMix,
        &mut chunk,
        ctx.out_channels,
    );
    ctx.out_pending.extend_from_slice(&chunk);

    write_pending(ctx)
}

fn process_audio_resampled(ctx: &mut DecodeContext) -> Result<bool, String> {
    while ctx.decode_pending.len() >= RESAMPLE_CHUNK_FRAMES * ctx.in_channels {
        let mut chunk_in: Vec<f32> = ctx
            .decode_pending
            .drain(..RESAMPLE_CHUNK_FRAMES * ctx.in_channels)
            .collect();

        apply_dsp_stage(
            ctx.dsp_chain,
            DspStage::PreMix,
            &mut chunk_in,
            ctx.in_channels,
        );

        let chunk = if ctx.in_channels == ctx.out_channels {
            chunk_in
        } else {
            adapt_channels_interleaved(
                &chunk_in,
                ctx.in_channels,
                ctx.out_channels,
                ctx.channel_mixer,
            )
        };

        let processed = resample_interleaved_chunk(
            ctx.resampler.as_mut().expect("checked"),
            &chunk,
            ctx.out_channels,
        )?;
        let mut processed = processed;

        apply_dsp_stage(
            ctx.dsp_chain,
            DspStage::PostMix,
            &mut processed,
            ctx.out_channels,
        );
        ctx.out_pending.extend_from_slice(&processed);

        if write_pending(ctx) {
            return Ok(true);
        }
        if ctx.pending_seek.is_some() {
            break;
        }
        if !*ctx.playing {
            break;
        }
    }
    Ok(false)
}

fn handle_eof_and_flush(ctx: &mut DecodeContext) -> bool {
    if let Some(resampler_inner) = ctx.resampler.as_mut() {
        if !ctx.decode_pending.is_empty() {
            ctx.decode_pending
                .resize(RESAMPLE_CHUNK_FRAMES * ctx.in_channels, 0.0);
            apply_dsp_stage(
                ctx.dsp_chain,
                DspStage::PreMix,
                ctx.decode_pending,
                ctx.in_channels,
            );

            let chunk = if ctx.in_channels == ctx.out_channels {
                ctx.decode_pending.clone()
            } else {
                adapt_channels_interleaved(
                    ctx.decode_pending,
                    ctx.in_channels,
                    ctx.out_channels,
                    ctx.channel_mixer,
                )
            };
            match resample_interleaved_chunk(resampler_inner, &chunk, ctx.out_channels) {
                Ok(mut processed) => {
                    apply_dsp_stage(
                        ctx.dsp_chain,
                        DspStage::PostMix,
                        &mut processed,
                        ctx.out_channels,
                    );
                    ctx.out_pending.extend_from_slice(&processed);
                    ctx.decode_pending.clear();
                }
                Err(e) => {
                    let _ = ctx.internal_tx.send(InternalMsg::Error(e));
                    return true;
                }
            }
        }
        while !ctx.out_pending.is_empty() {
            if write_pending(ctx) {
                return true;
            }
            if ctx.pending_seek.is_some() || !*ctx.playing {
                break;
            }
        }
    } else if !ctx.decode_pending.is_empty() {
        apply_dsp_stage(
            ctx.dsp_chain,
            DspStage::PreMix,
            ctx.decode_pending,
            ctx.in_channels,
        );

        let mut chunk = if ctx.in_channels == ctx.out_channels {
            std::mem::take(ctx.decode_pending)
        } else {
            let v = adapt_channels_interleaved(
                ctx.decode_pending,
                ctx.in_channels,
                ctx.out_channels,
                ctx.channel_mixer,
            );
            ctx.decode_pending.clear();
            v
        };

        apply_dsp_stage(
            ctx.dsp_chain,
            DspStage::PostMix,
            &mut chunk,
            ctx.out_channels,
        );
        ctx.out_pending.extend_from_slice(&chunk);

        while !ctx.out_pending.is_empty() {
            if write_pending(ctx) {
                return true;
            }
            if ctx.pending_seek.is_some() || !*ctx.playing {
                break;
            }
        }
    }
    false
}

fn sync_dsp_chain(ctx: &mut DecodeContext, chain: Vec<RuntimeDspChainEntry>) -> Result<(), String> {
    apply_or_recreate_dsp_chain(
        ctx.dsp_chain,
        &chain,
        ctx.in_channels,
        ctx.target_sample_rate,
        ctx.out_channels as u16,
    )
}
