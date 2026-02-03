use thiserror::Error;

#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("not implemented")]
    NotImplemented,
}

/// Decoding backend (planned: Symphonia wrapper).
///
/// In the real pipeline, this would:
/// - open container/codec for the selected track
/// - produce decoded PCM frames (likely `f32` interleaved)
pub trait Decoder: Send {
    fn open(&mut self, _path: &str) -> Result<(), DecodeError> {
        Err(DecodeError::NotImplemented)
    }

    /// Decode into `out` (interleaved PCM). Returns number of samples written.
    fn decode_next(&mut self, _out: &mut [f32]) -> Result<usize, DecodeError> {
        Err(DecodeError::NotImplemented)
    }
}

/// Stub decoder used while the real Symphonia integration is pending.
pub struct StubDecoder;

impl Decoder for StubDecoder {}
