use std::io;

use symphonia::core::errors::Error as SymphoniaError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("io error: {0}")]
    Io(#[from] io::Error),

    #[error("unsupported file extension for built-in decoder: `{ext}`")]
    UnsupportedExtension { ext: String },

    #[error("missing audio track")]
    MissingTrack,

    #[error("missing sample rate in codec parameters")]
    MissingSampleRate,

    #[error("decoder error: {0}")]
    Symphonia(#[from] SymphoniaError),
}
