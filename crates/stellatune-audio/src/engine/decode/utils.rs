use std::thread;
use std::time::Duration;

use crossbeam_channel::TrySendError;
use stellatune_mixer::ChannelMixer;
use stellatune_plugins::runtime::CapabilityKind as RuntimeCapabilityKind;
use tracing::warn;

use super::context::DecodeContext;
use super::decoder::{EngineDecoder, open_engine_decoder};
use super::dsp::apply_or_recreate_dsp_chain;
use super::resampler::create_resampler_if_needed;
use crate::engine::config::OUTPUT_SINK_WRITE_RETRY_SLEEP_MS;
use crate::engine::messages::{DecodeCtrl, InternalMsg};
use crate::engine::update_events::emit_config_update_runtime_event;

pub(crate) fn skip_frames_by_decoding(
    decoder: &mut EngineDecoder,
    mut frames_to_skip: u64,
) -> bool {
    while frames_to_skip > 0 {
        let want = (frames_to_skip.min(2048)) as usize;
        match decoder.next_block(want) {
            Ok(Some(block)) => {
                let got_frames = (block.len() / decoder.spec().channels as usize) as u64;
                if got_frames == 0 {
                    return false;
                }
                frames_to_skip = frames_to_skip.saturating_sub(got_frames);
            }
            Ok(None) => return false,
            Err(_) => return false,
        }
    }
    true
}

pub(crate) fn write_pending(ctx: &mut DecodeContext) -> bool {
    let mut offset = 0usize;
    while offset < ctx.out_pending.len() {
        while let Ok(ctrl) = ctx.ctrl_rx.try_recv() {
            match ctrl {
                DecodeCtrl::SetDspChain { chain } => {
                    if let Err(e) = apply_or_recreate_dsp_chain(
                        ctx.dsp_chain,
                        &chain,
                        ctx.in_channels,
                        ctx.target_sample_rate,
                        ctx.out_channels as u16,
                    ) {
                        let _ = ctx.internal_tx.send(InternalMsg::Error(e));
                    }
                }
                DecodeCtrl::SetLfeMode { mode } => {
                    *ctx.lfe_mode = core_lfe_to_mixer(mode);
                    *ctx.channel_mixer = stellatune_mixer::ChannelMixer::new(
                        stellatune_mixer::ChannelLayout::from_count(ctx.in_channels as u16),
                        stellatune_mixer::ChannelLayout::from_count(ctx.out_channels as u16),
                        *ctx.lfe_mode,
                    );
                }
                DecodeCtrl::Pause => {
                    *ctx.playing = false;
                    break;
                }
                DecodeCtrl::SeekMs { position_ms } => {
                    *ctx.pending_seek = Some(position_ms);
                    return false;
                }
                DecodeCtrl::SetOutputSinkTx {
                    tx,
                    output_sink_chunk_frames,
                } => {
                    *ctx.output_sink_tx = tx;
                    *ctx.output_sink_chunk_frames = output_sink_chunk_frames;
                }
                DecodeCtrl::RefreshDecoder => {
                    if let Err(e) = refresh_decoder(ctx) {
                        warn!("decoder refresh failed: {e}");
                    }
                }
                DecodeCtrl::Stop => return true,
                _ => {}
            }
        }
        if !*ctx.playing {
            break;
        }

        let written = if ctx.output_sink_only {
            let Some(tx) = ctx.output_sink_tx.as_ref() else {
                thread::sleep(Duration::from_millis(OUTPUT_SINK_WRITE_RETRY_SLEEP_MS));
                continue;
            };
            let preferred_samples = (*ctx.output_sink_chunk_frames as usize)
                .saturating_mul(ctx.out_channels)
                .max(ctx.out_channels);
            let remaining = ctx.out_pending.len().saturating_sub(offset);
            let chunk_len = if *ctx.output_sink_chunk_frames == 0 {
                remaining
            } else {
                preferred_samples.min(remaining)
            };
            let chunk = ctx.out_pending[offset..offset + chunk_len].to_vec();
            let chunk_len = chunk.len();
            match tx.try_send_samples(chunk) {
                Ok(()) => chunk_len,
                Err(TrySendError::Disconnected(_)) => {
                    *ctx.output_sink_tx = None;
                    thread::sleep(Duration::from_millis(OUTPUT_SINK_WRITE_RETRY_SLEEP_MS));
                    continue;
                }
                Err(TrySendError::Full(_)) => {
                    thread::sleep(Duration::from_millis(OUTPUT_SINK_WRITE_RETRY_SLEEP_MS));
                    continue;
                }
            }
        } else if let Ok(mut producer) = ctx.producer.lock() {
            producer.push_slice(&ctx.out_pending[offset..])
        } else {
            0
        };
        offset += written;
        *ctx.frames_written =
            (*ctx.frames_written).saturating_add((written / ctx.out_channels) as u64);
        if written == 0 {
            thread::sleep(Duration::from_millis(OUTPUT_SINK_WRITE_RETRY_SLEEP_MS));
        }
    }

    if offset > 0 {
        ctx.out_pending.drain(..offset);
    }

    false
}

fn active_decoder_generation(plugin_id: &str, type_id: &str) -> u64 {
    let shared = stellatune_plugins::shared_runtime_service();
    let Ok(service) = shared.lock() else {
        return 0;
    };
    service
        .resolve_active_capability(plugin_id, RuntimeCapabilityKind::Decoder, type_id)
        .map(|cap| cap.generation.0)
        .unwrap_or(0)
}

fn current_playback_position_ms(ctx: &DecodeContext) -> i64 {
    let buffered_frames = ctx
        .producer
        .lock()
        .map(|p| (p.len() / ctx.out_channels) as u64)
        .unwrap_or(0);
    let played_frames = ctx.frames_written.saturating_sub(buffered_frames);
    ctx.base_ms.saturating_add(
        ((played_frames.saturating_mul(1000)) / ctx.target_sample_rate.max(1) as u64) as i64,
    )
}

pub(crate) fn refresh_decoder(ctx: &mut DecodeContext) -> Result<(), String> {
    let target_ms = current_playback_position_ms(ctx).max(0);
    let (mut next_decoder, next_info) = match open_engine_decoder(ctx.path) {
        Ok(v) => v,
        Err(e) => {
            emit_config_update_runtime_event(
                "host.audio.decoder",
                "decoder",
                "refresh",
                "failed",
                0,
                Some(&e),
            );
            return Err(e);
        }
    };
    let plugin_id = next_info
        .decoder_plugin_id
        .clone()
        .unwrap_or_else(|| "host.audio.decoder".to_string());
    let type_id = next_info
        .decoder_type_id
        .clone()
        .unwrap_or_else(|| "refresh".to_string());
    let generation = if next_info.decoder_plugin_id.is_some() && next_info.decoder_type_id.is_some()
    {
        active_decoder_generation(&plugin_id, &type_id)
    } else {
        0
    };

    let next_spec = next_decoder.spec();
    if next_spec.sample_rate != ctx.spec_sample_rate
        || next_spec.channels as usize != ctx.in_channels
    {
        let message = format!(
            "decoder refresh changed format: {}Hz {}ch -> {}Hz {}ch",
            ctx.spec_sample_rate, ctx.in_channels, next_spec.sample_rate, next_spec.channels
        );
        emit_config_update_runtime_event(
            &plugin_id,
            "decoder",
            &type_id,
            "failed",
            generation,
            Some(&message),
        );
        return Err(message);
    }
    if target_ms > 0 {
        if let Err(e) = next_decoder.seek_ms(target_ms as u64) {
            let message = format!("decoder refresh seek failed: {e}");
            emit_config_update_runtime_event(
                &plugin_id,
                "decoder",
                &type_id,
                "failed",
                generation,
                Some(&message),
            );
            return Err(message);
        }
    }

    if let Ok(mut producer) = ctx.producer.lock() {
        producer.clear();
    }
    ctx.decode_pending.clear();
    ctx.out_pending.clear();
    *ctx.frames_written = 0;
    *ctx.base_ms = target_ms;
    *ctx.pending_seek = None;
    *ctx.resampler = create_resampler_if_needed(
        ctx.spec_sample_rate,
        ctx.target_sample_rate,
        ctx.out_channels,
    )?;
    *ctx.last_emit = std::time::Instant::now();
    *ctx.decoder = next_decoder;
    emit_config_update_runtime_event(
        &plugin_id,
        "decoder",
        &type_id,
        "recreated",
        generation,
        None,
    );
    Ok(())
}

pub(crate) fn core_lfe_to_mixer(mode: stellatune_core::LfeMode) -> stellatune_mixer::LfeMode {
    match mode {
        stellatune_core::LfeMode::Mute => stellatune_mixer::LfeMode::Mute,
        stellatune_core::LfeMode::MixToFront => stellatune_mixer::LfeMode::MixToFront,
    }
}

pub(crate) fn adapt_channels_interleaved(
    input: &[f32],
    in_channels: usize,
    out_channels: usize,
    mixer: &ChannelMixer,
) -> Vec<f32> {
    if in_channels == out_channels {
        return input.to_vec();
    }
    mixer.mix(input)
}
