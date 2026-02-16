use std::any::Any;

use crate::pipeline::error::PipelineError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputRef {
    TrackToken(String),
}

pub struct SourceHandle {
    inner: Box<dyn Any + Send>,
}

impl SourceHandle {
    pub fn new<T>(value: T) -> Self
    where
        T: Any + Send,
    {
        Self {
            inner: Box::new(value),
        }
    }

    pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
        (self.inner.as_ref() as &dyn Any).downcast_ref::<T>()
    }

    pub fn downcast_mut<T: Any>(&mut self) -> Option<&mut T> {
        (self.inner.as_mut() as &mut dyn Any).downcast_mut::<T>()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamSpec {
    pub sample_rate: u32,
    pub channels: u16,
}

impl StreamSpec {
    pub fn validate(self) -> Result<Self, PipelineError> {
        if self.sample_rate == 0 || self.channels == 0 {
            return Err(PipelineError::InvalidSpec {
                sample_rate: self.sample_rate,
                channels: self.channels,
            });
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackpressurePolicy {
    Drop,
    Retry,
    BlockForbidden,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageProfile {
    pub max_block_time_us: u32,
    pub no_alloc_hot_path: bool,
    pub backpressure_policy: BackpressurePolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecoderCapabilities {
    pub seek_supported: bool,
    pub seek_granularity_ms: u32,
    pub has_gapless_metadata: bool,
    pub preferred_block_frames: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GaplessTrimSpec {
    pub head_frames: u32,
    pub tail_frames: u32,
}

impl GaplessTrimSpec {
    pub fn is_disabled(self) -> bool {
        self.head_frames == 0 && self.tail_frames == 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionCurve {
    Linear,
    EqualPower,
}

impl Default for TransitionCurve {
    fn default() -> Self {
        Self::EqualPower
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionTimePolicy {
    Exact,
    FitToAvailable,
}

impl Default for TransitionTimePolicy {
    fn default() -> Self {
        Self::FitToAvailable
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MasterGainCurve {
    Linear,
    AudioTaper,
}

impl Default for MasterGainCurve {
    fn default() -> Self {
        Self::AudioTaper
    }
}

impl MasterGainCurve {
    const AUDIO_TAPER_MIN_DB: f32 = -60.0;
    const AUDIO_TAPER_EXPONENT: f32 = 2.0;

    pub fn level_to_gain(self, level: f32) -> f32 {
        let level = level.clamp(0.0, 1.0);
        if level <= 0.0 {
            return 0.0;
        }
        if level >= 1.0 {
            return 1.0;
        }

        match self {
            Self::Linear => level,
            // Common player behavior: slider value is mapped through a dB taper
            // to avoid "no effect here, huge jump there" feeling.
            Self::AudioTaper => {
                let attenuation = (1.0 - level).powf(Self::AUDIO_TAPER_EXPONENT);
                let db = Self::AUDIO_TAPER_MIN_DB * attenuation;
                10_f32.powf(db / 20.0).clamp(0.0, 1.0)
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GainTransitionRequest {
    pub target_gain: f32,
    pub ramp_ms: u32,
    pub available_frames_hint: Option<u64>,
    pub curve: TransitionCurve,
    pub time_policy: TransitionTimePolicy,
}

impl Default for GainTransitionRequest {
    fn default() -> Self {
        Self {
            target_gain: 1.0,
            ramp_ms: 0,
            available_frames_hint: None,
            curve: TransitionCurve::default(),
            time_policy: TransitionTimePolicy::default(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct AudioBlock {
    pub channels: u16,
    pub samples: Vec<f32>,
}

impl AudioBlock {
    pub fn new(channels: u16) -> Self {
        Self {
            channels,
            samples: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.samples.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    pub fn frames(&self) -> usize {
        let channels = self.channels.max(1) as usize;
        self.samples.len() / channels
    }
}

#[derive(Debug, Clone)]
pub struct PipelineContext {
    pub position_ms: i64,
    pub pending_seek_ms: Option<i64>,
}

impl Default for PipelineContext {
    fn default() -> Self {
        Self {
            position_ms: 0,
            pending_seek_ms: None,
        }
    }
}

impl PipelineContext {
    pub fn request_seek(&mut self, position_ms: i64) {
        self.pending_seek_ms = Some(position_ms.max(0));
    }

    pub fn clear_pending_seek(&mut self) -> Option<i64> {
        self.pending_seek_ms.take()
    }

    pub fn advance_frames(&mut self, frames: u64, sample_rate: u32) {
        if sample_rate == 0 {
            return;
        }
        let delta_ms = (frames.saturating_mul(1000)) / sample_rate as u64;
        self.position_ms = self.position_ms.saturating_add(delta_ms as i64);
    }
}

#[cfg(test)]
mod tests {
    use super::MasterGainCurve;

    #[test]
    fn audio_taper_curve_gives_finer_control_near_full_volume() {
        let curve = MasterGainCurve::AudioTaper;
        let near_top = curve.level_to_gain(0.9);
        let middle = curve.level_to_gain(0.5);
        let low = curve.level_to_gain(0.1);

        assert!(near_top > 0.92 && near_top < 0.95);
        assert!(middle > 0.17 && middle < 0.19);
        assert!(low > 0.0 && low < 0.005);
    }

    #[test]
    fn audio_taper_curve_is_monotonic() {
        let curve = MasterGainCurve::AudioTaper;
        let mut prev = 0.0;
        for i in 0..=100 {
            let level = i as f32 / 100.0;
            let gain = curve.level_to_gain(level);
            assert!(gain >= prev);
            prev = gain;
        }
    }
}
