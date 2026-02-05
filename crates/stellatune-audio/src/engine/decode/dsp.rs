use stellatune_plugins::DspInstance;

pub fn apply_dsp_chain(dsp_chain: &mut [DspInstance], samples: &mut [f32], out_channels: usize) {
    if dsp_chain.is_empty() || out_channels == 0 {
        return;
    }
    let frames = (samples.len() / out_channels) as u32;
    if frames == 0 {
        return;
    }
    for dsp in dsp_chain.iter_mut() {
        dsp.process_in_place(samples, frames);
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
    chain: Vec<DspInstance>,
    in_channels: usize,
) -> (Vec<DspInstance>, Vec<DspInstance>) {
    let in_layout = layout_to_flag(in_channels);
    let mut pre_mix = Vec::new();
    let mut post_mix = Vec::new();

    for dsp in chain {
        if dsp.supports_layout(in_layout) {
            pre_mix.push(dsp);
        } else {
            post_mix.push(dsp);
        }
    }
    (pre_mix, post_mix)
}
