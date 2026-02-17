use stellatune_audio_core::pipeline::context::{TransitionCurve, TransitionTimePolicy};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GainTransitionConfig {
    pub open_fade_in_ms: u32,
    pub play_fade_in_ms: u32,
    pub seek_fade_out_ms: u32,
    pub seek_fade_in_ms: u32,
    pub pause_fade_out_ms: u32,
    pub stop_fade_out_ms: u32,
    pub switch_fade_out_ms: u32,
    pub curve: TransitionCurve,
    pub fade_in_time_policy: TransitionTimePolicy,
    pub fade_out_time_policy: TransitionTimePolicy,
    pub interrupt_max_extra_wait_ms: u32,
}

impl Default for GainTransitionConfig {
    fn default() -> Self {
        Self {
            open_fade_in_ms: 24,
            play_fade_in_ms: 24,
            seek_fade_out_ms: 24,
            seek_fade_in_ms: 24,
            pause_fade_out_ms: 36,
            stop_fade_out_ms: 48,
            switch_fade_out_ms: 36,
            curve: TransitionCurve::EqualPower,
            fade_in_time_policy: TransitionTimePolicy::Exact,
            fade_out_time_policy: TransitionTimePolicy::FitToAvailable,
            interrupt_max_extra_wait_ms: 80,
        }
    }
}
