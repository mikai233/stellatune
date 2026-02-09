use stellatune_plugins::v2::DspInstanceV2;

pub fn apply_dsp_chain(dsp_chain: &mut [DspInstanceV2], samples: &mut [f32], out_channels: usize) {
    if dsp_chain.is_empty() || out_channels == 0 {
        return;
    }
    let frames = (samples.len() / out_channels) as u32;
    if frames == 0 {
        return;
    }
    for dsp in dsp_chain.iter_mut() {
        dsp.process_interleaved_f32_in_place(samples, frames);
    }
}

pub fn layout_to_flag(channels: usize) -> u32 {
    use stellatune_plugin_api::*;
    match channels {
        1 => ST_LAYOUT_MONO,
        2 => ST_LAYOUT_STEREO,
        6 => ST_LAYOUT_5_1,
        8 => ST_LAYOUT_7_1,
        _ => ST_LAYOUT_STEREO,
    }
}

pub fn split_dsp_chain_by_layout(
    chain: Vec<DspInstanceV2>,
    in_channels: usize,
) -> (Vec<DspInstanceV2>, Vec<DspInstanceV2>) {
    let in_layout = layout_to_flag(in_channels);
    let mut pre_mix = Vec::new();
    let mut post_mix = Vec::new();

    for dsp in chain {
        let supported = dsp.supported_layouts();
        if supported == stellatune_plugin_api::ST_LAYOUT_ANY || (supported & in_layout) != 0 {
            pre_mix.push(dsp);
        } else {
            post_mix.push(dsp);
        }
    }
    (pre_mix, post_mix)
}
