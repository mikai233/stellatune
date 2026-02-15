use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::{Duration, Instant};

use crossbeam_channel::RecvTimeoutError;
use stellatune_mixer::{ChannelLayout, ChannelMixer};
use tracing::{debug, warn};

use crate::engine::control::{internal_error_dispatch, internal_position_dispatch};
use crate::engine::messages::{DecodeCtrl, DecodeWorkerState};

use super::audio_path::sync_dsp_chain;
use super::context::DecodeContext;
use super::dsp::apply_runtime_control_updates;
use super::resampler::create_resampler_if_needed;
use super::utils::{core_lfe_to_mixer, refresh_decoder};

pub(super) fn set_decode_worker_state(
    runtime_state: &Arc<AtomicU8>,
    next: DecodeWorkerState,
    reason: &str,
) {
    let prev = runtime_state.swap(next as u8, Ordering::Relaxed);
    if prev == next as u8 {
        return;
    }
    let prev = DecodeWorkerState::from_u8(prev);
    debug!(from = ?prev, to = ?next, reason, "decode worker state");
}

pub(super) fn perform_seek(target_ms: i64, ctx: &mut DecodeContext) -> Result<(), String> {
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
        ctx.resample_quality,
    )?;
    *ctx.last_emit = Instant::now();
    Ok(())
}

pub(super) fn handle_paused_controls(
    ctx: &mut DecodeContext,
    runtime_state: &Arc<AtomicU8>,
) -> bool {
    if maybe_refresh_decoder_from_runtime_control(ctx) {
        return false;
    }
    if let Err(e) = apply_runtime_control_updates(
        ctx.dsp_chain,
        ctx.in_channels,
        ctx.target_sample_rate,
        ctx.out_channels as u16,
    ) {
        let _ = ctx.internal_tx.send(internal_error_dispatch(e));
        *ctx.playing = false;
        return false;
    }

    match ctx.ctrl_rx.recv_timeout(Duration::from_millis(20)) {
        Ok(DecodeCtrl::Play) => {
            *ctx.playing = true;
            *ctx.last_emit = Instant::now();
            set_decode_worker_state(runtime_state, DecodeWorkerState::Playing, "play");
        },
        Ok(DecodeCtrl::Pause) => {
            set_decode_worker_state(runtime_state, DecodeWorkerState::Paused, "pause");
        },
        Ok(DecodeCtrl::SetDspChain { chain }) => {
            if let Err(e) = sync_dsp_chain(ctx, chain) {
                let _ = ctx.internal_tx.send(internal_error_dispatch(e));
            }
        },
        Ok(DecodeCtrl::SeekMs { position_ms }) => {
            if let Err(e) = perform_seek(position_ms, ctx) {
                let _ = ctx.internal_tx.send(internal_error_dispatch(e));
            }
        },
        Ok(DecodeCtrl::SetLfeMode { mode }) => {
            *ctx.lfe_mode = core_lfe_to_mixer(mode);
            *ctx.channel_mixer = ChannelMixer::new(
                ChannelLayout::from_count(ctx.in_channels as u16),
                ChannelLayout::from_count(ctx.out_channels as u16),
                *ctx.lfe_mode,
            );
        },
        Ok(DecodeCtrl::SetOutputSinkTx {
            tx,
            output_sink_chunk_frames,
        }) => {
            *ctx.output_sink_tx = tx;
            *ctx.output_sink_chunk_frames = output_sink_chunk_frames;
        },
        Ok(DecodeCtrl::Stop) | Err(RecvTimeoutError::Disconnected) => {
            set_decode_worker_state(runtime_state, DecodeWorkerState::Idle, "stop");
            return true;
        },
        Err(RecvTimeoutError::Timeout) => {},
        _ => {},
    }
    false
}

pub(super) fn handle_playing_controls(
    ctx: &mut DecodeContext,
    runtime_state: &Arc<AtomicU8>,
) -> bool {
    if maybe_refresh_decoder_from_runtime_control(ctx) {
        return false;
    }
    if let Err(e) = apply_runtime_control_updates(
        ctx.dsp_chain,
        ctx.in_channels,
        ctx.target_sample_rate,
        ctx.out_channels as u16,
    ) {
        let _ = ctx.internal_tx.send(internal_error_dispatch(e));
        *ctx.playing = false;
        return false;
    }

    while let Ok(ctrl) = ctx.ctrl_rx.try_recv() {
        match ctrl {
            DecodeCtrl::SetLfeMode { mode } => {
                *ctx.lfe_mode = core_lfe_to_mixer(mode);
                *ctx.channel_mixer = ChannelMixer::new(
                    ChannelLayout::from_count(ctx.in_channels as u16),
                    ChannelLayout::from_count(ctx.out_channels as u16),
                    *ctx.lfe_mode,
                );
            },
            DecodeCtrl::Pause => {
                *ctx.playing = false;
                set_decode_worker_state(runtime_state, DecodeWorkerState::Paused, "pause");
                return false;
            },
            DecodeCtrl::SeekMs { position_ms } => {
                if let Err(e) = perform_seek(position_ms, ctx) {
                    let _ = ctx.internal_tx.send(internal_error_dispatch(e));
                    *ctx.playing = false;
                }
                return false;
            },
            DecodeCtrl::SetDspChain { chain } => {
                if let Err(e) = sync_dsp_chain(ctx, chain) {
                    let _ = ctx.internal_tx.send(internal_error_dispatch(e));
                }
            },
            DecodeCtrl::SetOutputSinkTx {
                tx,
                output_sink_chunk_frames,
            } => {
                *ctx.output_sink_tx = tx;
                *ctx.output_sink_chunk_frames = output_sink_chunk_frames;
            },
            DecodeCtrl::Stop => {
                set_decode_worker_state(runtime_state, DecodeWorkerState::Idle, "stop");
                return true;
            },
            _ => {},
        }
    }
    false
}

fn maybe_refresh_decoder_from_runtime_control(ctx: &mut DecodeContext) -> bool {
    if !ctx.decoder.has_pending_runtime_recreate() {
        return false;
    }
    if let Err(e) = refresh_decoder(ctx) {
        warn!("decoder refresh on worker control failed: {e}");
        let _ = ctx.internal_tx.send(internal_error_dispatch(e));
        *ctx.playing = false;
    }
    true
}

pub(super) fn emit_position(ctx: &mut DecodeContext) {
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
        let _ = ctx
            .internal_tx
            .try_send(internal_position_dispatch(ctx.path.to_string(), ms));
        *ctx.last_emit = Instant::now();
    }
}
