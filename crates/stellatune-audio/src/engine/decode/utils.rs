use std::thread;
use std::time::Duration;

use stellatune_mixer::ChannelMixer;

use super::context::DecodeContext;
use super::decoder::EngineDecoder;
use super::dsp::split_dsp_chain_by_layout;
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
                DecodeCtrl::Stop => return true,
                _ => {}
            }
        }
        if !*ctx.playing {
            break;
        }

        let written = ctx.producer.push_slice(&ctx.out_pending[offset..]);
        offset += written;
        *ctx.frames_written =
            (*ctx.frames_written).saturating_add((written / ctx.out_channels) as u64);
        if written == 0 {
            thread::sleep(Duration::from_millis(2));
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
