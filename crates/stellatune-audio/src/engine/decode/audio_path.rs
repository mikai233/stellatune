use crate::engine::config::RESAMPLE_CHUNK_FRAMES;
use crate::engine::control::internal_error_dispatch;
use crate::engine::messages::RuntimeDspChainEntry;

use super::context::DecodeContext;
use super::dsp::{DspStage, apply_dsp_stage, apply_or_recreate_dsp_chain};
use super::resampler::resample_interleaved_chunk;
use super::utils::{adapt_channels_interleaved, write_pending};

pub(super) fn process_audio_no_resample(ctx: &mut DecodeContext) -> bool {
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

pub(super) fn process_audio_resampled(ctx: &mut DecodeContext) -> Result<bool, String> {
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

pub(super) fn handle_eof_and_flush(ctx: &mut DecodeContext) -> bool {
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
                },
                Err(e) => {
                    let _ = ctx.internal_tx.send(internal_error_dispatch(e));
                    return true;
                },
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

pub(super) fn sync_dsp_chain(
    ctx: &mut DecodeContext,
    chain: Vec<RuntimeDspChainEntry>,
) -> Result<(), String> {
    apply_or_recreate_dsp_chain(
        ctx.dsp_chain,
        &chain,
        ctx.in_channels,
        ctx.target_sample_rate,
        ctx.out_channels as u16,
    )
}
