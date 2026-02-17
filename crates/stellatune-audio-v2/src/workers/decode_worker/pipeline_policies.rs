use crate::assembly::AssembledPipeline;
use crate::workers::decode_worker::state::DecodeWorkerState;

pub(crate) fn apply_decode_policies(assembled: &mut AssembledPipeline, state: &DecodeWorkerState) {
    if let Some(mixer) = assembled.decode.mixer.as_mut() {
        mixer.lfe_mode = state.lfe_mode;
    }
    if let Some(resampler) = assembled.decode.resampler.as_mut() {
        resampler.quality = state.resample_quality;
    }
}
