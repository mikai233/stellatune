use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use arc_swap::ArcSwap;
use stellatune_audio_core::pipeline::context::MasterGainCurve;

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
