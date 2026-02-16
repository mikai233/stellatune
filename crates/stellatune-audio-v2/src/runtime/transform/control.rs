use stellatune_audio_core::pipeline::context::{
    GainTransitionRequest, GaplessTrimSpec, MasterGainCurve,
};

pub(crate) const GAPLESS_TRIM_STAGE_KEY: &str = "builtin.gapless_trim";
pub(crate) const TRANSITION_GAIN_STAGE_KEY: &str = "builtin.transition_gain";
pub(crate) const MASTER_GAIN_STAGE_KEY: &str = "builtin.master_gain";

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MasterGainControl {
    pub level: f32,
    pub curve: Option<MasterGainCurve>,
}

impl MasterGainControl {
    pub(crate) fn new(level: f32) -> Self {
        Self {
            level: level.clamp(0.0, 1.0),
            curve: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_curve(level: f32, curve: MasterGainCurve) -> Self {
        Self {
            level: level.clamp(0.0, 1.0),
            curve: Some(curve),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TransitionGainControl {
    pub request: GainTransitionRequest,
}

impl TransitionGainControl {
    pub(crate) fn new(request: GainTransitionRequest) -> Self {
        Self { request }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GaplessTrimControl {
    pub spec: Option<GaplessTrimSpec>,
    pub position_ms: i64,
}

impl GaplessTrimControl {
    pub(crate) fn new(spec: Option<GaplessTrimSpec>, position_ms: i64) -> Self {
        Self {
            spec: spec.filter(|v| !v.is_disabled()),
            position_ms: position_ms.max(0),
        }
    }
}
