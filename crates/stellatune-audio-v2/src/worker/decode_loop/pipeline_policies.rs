use crate::assembly::AssembledPipeline;
use crate::worker::decode_loop::loop_state::DecodeLoopState;

pub(crate) fn apply_decode_policies(assembled: &mut AssembledPipeline, state: &DecodeLoopState) {
    if let Some(mixer) = assembled.decode.mixer.as_mut() {
        mixer.lfe_mode = state.lfe_mode;
    }
    if let Some(resampler) = assembled.decode.resampler.as_mut() {
        resampler.quality = state.resample_quality;
    }
}
