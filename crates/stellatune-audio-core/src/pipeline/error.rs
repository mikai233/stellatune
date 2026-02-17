use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum PipelineError {
    #[error("invalid stream spec: sample_rate={sample_rate} channels={channels}")]
    InvalidSpec { sample_rate: u32, channels: u16 },
    #[error("source unavailable")]
    SourceUnavailable,
    #[error("decoder unavailable")]
    DecoderUnavailable,
    #[error("sink disconnected")]
    SinkDisconnected,
    #[error("pipeline not prepared")]
    NotPrepared,
    #[error("stage failure: {0}")]
    StageFailure(String),
}

impl From<String> for PipelineError {
    fn from(value: String) -> Self {
        Self::StageFailure(value)
    }
}
