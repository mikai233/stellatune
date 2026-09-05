//! Audio output stages, clocks, and output compatibility.
//!
//! Sink writes are synchronous but bounded. A sink may consume only a prefix
//! of a block and report
//! [`SinkWriteState::WouldBlock`](crate::sink::SinkWriteState::WouldBlock); the
//! dedicated sink worker retains the remainder and retries it without blocking
//! playback control.

use crate::{
    error::{FactoryError, SinkError},
    format::{AudioBlock, ChannelLayout, PcmFormat},
    stage::StageId,
};

/// Whether a sink can immediately accept another write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SinkWriteState {
    /// The sink may be called again without waiting for device progress.
    Ready,
    /// The sink accepted as much as possible and is temporarily backpressured.
    WouldBlock,
}

/// The number of input frames accepted by one sink write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SinkWriteResult {
    /// Frames consumed from the beginning of the supplied block.
    pub consumed_frames: usize,
    /// Whether the sink currently has capacity for another write.
    pub state: SinkWriteState,
}

/// A snapshot of device-consumed and queued output frames.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SinkClockSnapshot {
    /// Frames that have reached the device playback position.
    pub consumed_frames: u64,
    /// Frames accepted by the sink but not yet consumed by the device.
    pub buffered_frames: u64,
    /// The sink-local discontinuity generation.
    ///
    /// Discarding queued output advances this value so callers can reject a
    /// clock snapshot from an older stream position.
    pub epoch: u64,
}

/// A stateful audio output endpoint driven by a dedicated worker thread.
///
/// The runtime opens a sink once, writes whole or partial blocks, and closes it
/// during deterministic shutdown. Control methods may be called between
/// writes. Implementations must keep [`Self::write`] bounded and express device
/// backpressure through [`SinkWriteResult`].
pub trait SinkStage: Send {
    /// Configures software buffering before opening the output endpoint.
    fn configure_buffering(&mut self, _config: crate::buffering::BufferingConfig) {}
    /// Opens the output endpoint for an exact PCM format.
    ///
    /// # Errors
    ///
    /// Returns [`SinkError::Unsupported`] when the format or route cannot be
    /// opened, or another [`SinkError`] for device initialization failures.
    fn open(&mut self, format: PcmFormat) -> Result<(), SinkError>;
    /// Attempts to write a prefix of `block` to the endpoint.
    ///
    /// `consumed_frames` must not exceed [`AudioBlock::frames`]. Returning zero
    /// frames with [`SinkWriteState::WouldBlock`] is valid.
    ///
    /// # Errors
    ///
    /// Returns a [`SinkError`] for invalid input, device I/O, or route loss.
    fn write(&mut self, block: &AudioBlock) -> Result<SinkWriteResult, SinkError>;
    /// Pauses device playback without discarding queued frames.
    ///
    /// # Errors
    ///
    /// Returns a [`SinkError`] when the endpoint cannot be paused.
    fn pause(&mut self) -> Result<(), SinkError>;
    /// Resumes device playback after a pause.
    ///
    /// # Errors
    ///
    /// Returns a [`SinkError`] when the endpoint cannot be resumed.
    fn resume(&mut self) -> Result<(), SinkError>;
    /// Waits for accepted frames to reach the device playback boundary.
    ///
    /// This method runs on the sink worker rather than the playback actor.
    ///
    /// # Errors
    ///
    /// Returns a [`SinkError`] when queued output cannot be drained.
    fn drain(&mut self) -> Result<(), SinkError>;
    /// Discards queued output and establishes a new sink clock epoch.
    ///
    /// # Errors
    ///
    /// Returns a [`SinkError`] when queued device data cannot be invalidated.
    fn discard(&mut self) -> Result<(), SinkError>;
    /// Returns a non-blocking snapshot of the output device clock.
    fn clock_snapshot(&self) -> SinkClockSnapshot;
    /// Releases the output endpoint and its device resources.
    fn close(&mut self);
}

/// Properties that determine whether an existing sink can be reused.
///
/// Equality requires the exact positioned channel layout, not only an equal
/// number of channels.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OutputCompatibilityKey {
    /// The stable output backend identifier.
    pub backend_id: String,
    /// The selected endpoint identifier, or the backend's default endpoint.
    pub device_id: Option<String>,
    /// The negotiated output sample rate.
    pub sample_rate: u32,
    /// The negotiated positioned output layout.
    pub channel_layout: ChannelLayout,
    /// A revision that changes whenever routing configuration invalidates reuse.
    pub route_revision: u64,
}

/// Negotiates output formats and creates independent sink stages.
pub trait SinkFactory: Send + Sync {
    /// Returns the stable output stage identifier.
    fn id(&self) -> &StageId;
    /// Returns the exact output format preferred for `input`.
    ///
    /// The default implementation preserves the input format.
    ///
    /// # Errors
    ///
    /// Returns [`FactoryError`] when no safe output format can be negotiated.
    fn preferred_format(&self, input: PcmFormat) -> Result<PcmFormat, FactoryError> {
        Ok(input)
    }
    /// Computes the sink-reuse key for a negotiated format.
    ///
    /// # Errors
    ///
    /// Returns [`FactoryError`] when the active route cannot represent
    /// `format` or its compatibility identity cannot be determined.
    fn compatibility_key(&self, format: PcmFormat) -> Result<OutputCompatibilityKey, FactoryError>;
    /// Creates a fresh, unopened sink stage.
    ///
    /// # Errors
    ///
    /// Returns [`FactoryError`] when configuration is invalid or the endpoint
    /// instance cannot be created.
    fn create(&self) -> Result<Box<dyn SinkStage>, FactoryError>;
}
