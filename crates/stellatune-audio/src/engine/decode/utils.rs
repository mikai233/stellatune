use std::thread;
use std::time::Duration;

use crossbeam_channel::TrySendError;
use stellatune_mixer::ChannelMixer;

use super::context::DecodeContext;
use super::decoder::EngineDecoder;
use super::dsp::split_dsp_chain_by_layout;
use crate::engine::config::OUTPUT_SINK_WRITE_RETRY_SLEEP_MS;
use crate::engine::messages::DecodeCtrl;

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
                    let (pre, post) = split_dsp_chain_by_layout(chain, ctx.in_channels);
                    *ctx.pre_mix_dsp = pre;
                    *ctx.post_mix_dsp = post;
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
