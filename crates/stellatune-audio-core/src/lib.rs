#![deny(clippy::wildcard_imports)]

pub mod contracts;
pub mod decoder;
pub mod errors;
pub mod sink;
pub mod source;
pub mod transform;

pub use contracts::{
    AudioBlock, AudioFormat, BlockTimeline, MediaHints, MediaTime, PlaybackItem, PlaybackItemId,
    SourceCapabilities, SourceDescriptor, StageId,
};
pub use decoder::{
    DecodeStatus, DecodedStreamInfo, DecoderDescriptor, DecoderFactory, DecoderSeekStatus,
    DecoderStage, GaplessTrimSpec, SeekResult,
};
pub use errors::{
    DecodeError, FactoryError, FailureStage, PlaybackControlError, PlaybackFailure,
    RetryDisposition, SinkError, SourceError, TransformError,
};
pub use sink::{
    OutputCompatibilityKey, SinkClockSnapshot, SinkFactory, SinkStage, SinkWriteResult,
    SinkWriteState,
};
pub use source::{
    EncodedSource, MemorySourceFactory, SourceCancellation, SourceFactory, SourceOpenFuture,
    SourceOpenPurpose, SourceOpenRequest,
};
pub use transform::{
    DrainStatus, TransformDescriptor, TransformFactory, TransformPlacement, TransformStage,
    TransformStatus,
};
