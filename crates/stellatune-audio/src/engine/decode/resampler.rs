use crate::engine::config::{
    RESAMPLE_CHUNK_FRAMES, RESAMPLE_CUTOFF, RESAMPLE_INTERPOLATION, RESAMPLE_OVERSAMPLING_FACTOR,
    RESAMPLE_SINC_LEN, RESAMPLE_WINDOW,
};

pub fn create_resampler_if_needed(
    src_rate: u32,
    dst_rate: u32,
    channels: usize,
) -> Result<Option<rubato::Async<f32>>, String> {
    if src_rate == dst_rate {
        return Ok(None);
    }

    use rubato::{Async, FixedAsync, SincInterpolationParameters};

    let params = SincInterpolationParameters {
        sinc_len: RESAMPLE_SINC_LEN,
        f_cutoff: RESAMPLE_CUTOFF,
        oversampling_factor: RESAMPLE_OVERSAMPLING_FACTOR,
        interpolation: RESAMPLE_INTERPOLATION,
        window: RESAMPLE_WINDOW,
    };

    let ratio = dst_rate as f64 / src_rate as f64;
    let resampler = Async::<f32>::new_sinc(
        ratio,
        2.0,
        &params,
        RESAMPLE_CHUNK_FRAMES,
        channels,
        FixedAsync::Input,
    )
    .map_err(|e| format!("failed to create resampler: {e}"))?;
    Ok(Some(resampler))
}

pub fn resample_interleaved_chunk(
    resampler: &mut rubato::Async<f32>,
    chunk_interleaved: &[f32],
    channels: usize,
) -> Result<Vec<f32>, String> {
    use audioadapter_buffers::direct::InterleavedSlice;
    use rubato::Resampler;

    let frames = chunk_interleaved.len() / channels;
    let input = InterleavedSlice::new(chunk_interleaved, channels, frames)
        .map_err(|e| format!("resample input buffer error: {e}"))?;

    let out = resampler
        .process(&input, 0, None)
        .map_err(|e| format!("resample error: {e}"))?;

    Ok(out.take_data())
}
