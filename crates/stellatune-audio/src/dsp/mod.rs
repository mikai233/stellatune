/// DSP stage (planned: gain/resample/EQ chain).
pub trait DspStage: Send {
    fn process(&mut self, _samples: &mut [f32]) {}
}

/// No-op DSP stage placeholder.
pub struct NoopDsp;

impl DspStage for NoopDsp {}
