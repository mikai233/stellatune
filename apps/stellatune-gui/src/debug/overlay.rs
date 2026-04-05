use std::time::Instant;

#[derive(Debug)]
pub struct DebugOverlay {
    last_sample_at: Option<Instant>,
    smoothed_fps: f32,
}

impl Default for DebugOverlay {
    fn default() -> Self {
        Self {
            last_sample_at: None,
            smoothed_fps: 60.0,
        }
    }
}

impl DebugOverlay {
    pub fn observe_frame(&mut self, now: Instant) -> f32 {
        let instantaneous_fps = self
            .last_sample_at
            .map(|last| now.saturating_duration_since(last).as_secs_f32())
            .filter(|delta| *delta > 0.0)
            .map(|delta| 1.0 / delta)
            .unwrap_or(self.smoothed_fps);
        self.last_sample_at = Some(now);
        self.smoothed_fps = (self.smoothed_fps * 0.9) + (instantaneous_fps * 0.1);
        self.smoothed_fps
    }
}
