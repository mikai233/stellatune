use stellatune_audio_core::pipeline::context::{TransitionCurve, TransitionTimePolicy};

/// Fade/ramp policy for playback gain transitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GainTransitionConfig {
    /// Fade-in duration applied when a new track is opened.
    pub open_fade_in_ms: u32,
    /// Fade-in duration applied when transitioning from paused to playing.
    pub play_fade_in_ms: u32,
    /// Fade-out duration applied before seek repositioning.
    pub seek_fade_out_ms: u32,
    /// Fade-in duration applied after seek repositioning.
    pub seek_fade_in_ms: u32,
    /// Fade-out duration applied before pause.
    pub pause_fade_out_ms: u32,
    /// Fade-out duration applied before stop.
    pub stop_fade_out_ms: u32,
    /// Fade-out duration applied before track switch.
    pub switch_fade_out_ms: u32,
    /// Curve shape used by gain ramps.
    pub curve: TransitionCurve,
    /// Time policy for fade-in transitions.
    pub fade_in_time_policy: TransitionTimePolicy,
    /// Time policy for fade-out transitions.
    pub fade_out_time_policy: TransitionTimePolicy,
    /// Extra wait budget for interrupted transitions.
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
