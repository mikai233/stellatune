use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use arc_swap::ArcSwap;
use stellatune_audio_core::pipeline::context::{
    GainTransitionRequest, GaplessTrimSpec, MasterGainCurve,
};

pub(crate) const GAPLESS_TRIM_STAGE_KEY: &str = "builtin.gapless_trim";
pub(crate) const TRANSITION_GAIN_STAGE_KEY: &str = "builtin.transition_gain";
pub(crate) const MASTER_GAIN_STAGE_KEY: &str = "builtin.master_gain";

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MasterGainHotState {
    pub level: f32,
    pub ramp_ms: u32,
    pub curve: Option<MasterGainCurve>,
}

impl Default for MasterGainHotState {
    fn default() -> Self {
        Self {
            level: 1.0,
            ramp_ms: 0,
            curve: None,
        }
    }
}

#[derive(Debug)]
pub(crate) struct MasterGainHotControl {
    state: ArcSwap<MasterGainHotState>,
    version: AtomicU64,
}

pub(crate) type SharedMasterGainHotControl = Arc<MasterGainHotControl>;

impl Default for MasterGainHotControl {
    fn default() -> Self {
        Self::new(MasterGainHotState::default())
    }
}

impl MasterGainHotControl {
    pub(crate) fn new(initial: MasterGainHotState) -> Self {
        Self {
            state: ArcSwap::from_pointee(initial),
            version: AtomicU64::new(0),
        }
    }

    pub(crate) fn update(&self, level: f32, ramp_ms: u32, curve: Option<MasterGainCurve>) -> u64 {
        let next = MasterGainHotState {
            level: level.clamp(0.0, 1.0),
            ramp_ms,
            curve,
        };
        self.state.store(Arc::new(next));
        self.version
            .fetch_add(1, Ordering::AcqRel)
            .saturating_add(1)
    }

    pub(crate) fn snapshot(&self) -> MasterGainHotState {
        **self.state.load()
    }

    pub(crate) fn version(&self) -> u64 {
        self.version.load(Ordering::Acquire)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MasterGainControl {
    pub level: f32,
    pub ramp_ms: u32,
    pub curve: Option<MasterGainCurve>,
}

impl MasterGainControl {
    pub(crate) fn new(level: f32, ramp_ms: u32) -> Self {
        Self {
            level: level.clamp(0.0, 1.0),
            ramp_ms,
            curve: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_curve(level: f32, ramp_ms: u32, curve: MasterGainCurve) -> Self {
        Self {
            level: level.clamp(0.0, 1.0),
            ramp_ms,
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
