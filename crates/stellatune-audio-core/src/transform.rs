//! Stateful PCM transforms and their position in the mixing pipeline.
//!
//! A transform is configured with an input format before processing. It may
//! change that format and may buffer samples internally. After upstream input
//! ends, the runtime repeatedly calls
//! [`TransformStage::drain`](crate::transform::TransformStage::drain) before
//! moving to the next stage or finishing the track.

use crate::{
    error::{FactoryError, TransformError},
    format::{AudioBlock, PcmFormat},
    stage::StageId,
};

/// The result of processing one input block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformStatus {
    /// The block contains output ready for the next pipeline stage.
    Produced,
    /// The input was accepted but no output is available in this turn.
    Buffered,
}

/// The result of draining a transform after end of input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrainStatus {
    /// The supplied output block contains buffered PCM.
    Produced,
    /// The transform has no buffered PCM remaining.
    Complete,
}

/// A stateful, in-place PCM processing stage.
///
/// The runtime calls [`Self::configure`] before [`Self::process`], then drains
/// buffered output to completion and calls [`Self::reset`] before reuse.
pub trait TransformStage: Send {
    /// Configures the stage and returns the format it will produce.
    ///
    /// # Errors
    ///
    /// Returns [`TransformError::Unsupported`] when `input` cannot be handled,
    /// or another [`TransformError`] when configuration fails.
    fn configure(&mut self, input: PcmFormat) -> Result<PcmFormat, TransformError>;
    /// Processes a PCM block in place.
    ///
    /// On [`TransformStatus::Produced`], the block's samples use the output
    /// format returned by [`Self::configure`]. On
    /// [`TransformStatus::Buffered`], the runtime treats the block as having no
    /// output for this turn.
    ///
    /// # Errors
    ///
    /// Returns a [`TransformError`] when the input is invalid or processing
    /// cannot continue.
    fn process(&mut self, block: &mut AudioBlock) -> Result<TransformStatus, TransformError>;
    /// Produces at most one block of buffered output after upstream EOF.
    ///
    /// The runtime calls this method repeatedly until it returns
    /// [`DrainStatus::Complete`].
    ///
    /// # Errors
    ///
    /// Returns a [`TransformError`] when buffered output cannot be produced.
    fn drain(&mut self, output: &mut AudioBlock) -> Result<DrainStatus, TransformError>;
    /// Clears all format-specific and buffered state.
    fn reset(&mut self);
}

/// The side of the track mixer on which a transform runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TransformPlacement {
    /// Runs independently on each track before gain and track mixing.
    PreMix,
    /// Runs once on the shared stream after track mixing.
    PostMix,
}

/// Stable identity and placement metadata for a transform factory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransformDescriptor {
    /// The stable transform implementation identifier.
    pub id: StageId,
    /// The transform's position relative to the track mixer.
    pub placement: TransformPlacement,
}

/// Creates independent PCM transform stages.
pub trait TransformFactory: Send + Sync {
    /// Returns the factory's stable descriptor.
    fn descriptor(&self) -> &TransformDescriptor;
    /// Creates a fresh, unconfigured transform stage.
    ///
    /// # Errors
    ///
    /// Returns [`FactoryError`] when configuration is invalid or the transform
    /// instance cannot be created.
    fn create(&self) -> Result<Box<dyn TransformStage>, FactoryError>;
}
