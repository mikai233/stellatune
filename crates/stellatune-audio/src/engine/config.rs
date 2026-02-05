use std::time::Duration;

pub(crate) const CONTROL_TICK_MS: u64 = 50;
pub(crate) const RING_BUFFER_CAPACITY_MS: usize = 500;
pub(crate) const BUFFER_LOW_WATERMARK_MS: i64 = 60;
pub(crate) const BUFFER_HIGH_WATERMARK_MS: i64 = 200;
// More conservative buffer watermarks for WASAPI exclusive mode. This increases latency but
// reduces sensitivity to scheduling jitter and decode/resample spikes.
pub(crate) const BUFFER_LOW_WATERMARK_MS_EXCLUSIVE: i64 = 120;
pub(crate) const BUFFER_HIGH_WATERMARK_MS_EXCLUSIVE: i64 = 400;
// While output is gated during Buffering, don't overfill the ring buffer. This prevents a burst
// of decode+resample CPU work right before audio becomes audible.
pub(crate) const BUFFER_PREFILL_CAP_MS: i64 = BUFFER_HIGH_WATERMARK_MS + 50;
pub(crate) const BUFFER_PREFILL_CAP_MS_EXCLUSIVE: i64 = BUFFER_HIGH_WATERMARK_MS_EXCLUSIVE + 50;
pub(crate) const UNDERRUN_LOG_INTERVAL: Duration = Duration::from_secs(1);

pub(crate) const RESAMPLE_CHUNK_FRAMES: usize = 1024;
// High-quality resampler preset.
//
// Notes:
// - This is intentionally CPU-heavier but should sound better.
// - We'll add a user-configurable quality level later.
pub(crate) const RESAMPLE_SINC_LEN: usize = 256;
pub(crate) const RESAMPLE_CUTOFF: f32 = 0.95;
pub(crate) const RESAMPLE_OVERSAMPLING_FACTOR: usize = 128;
pub(crate) const RESAMPLE_WINDOW: rubato::WindowFunction = rubato::WindowFunction::BlackmanHarris2;
pub(crate) const RESAMPLE_INTERPOLATION: rubato::SincInterpolationType =
    rubato::SincInterpolationType::Linear;
