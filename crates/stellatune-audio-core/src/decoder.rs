use crate::{AudioBlock, DecodeError, EncodedSource, FactoryError, MediaHints, PcmFormat, StageId};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GaplessTrimSpec {
    pub head_frames: u32,
    pub tail_frames: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedStreamInfo {
    pub format: PcmFormat,
    pub duration_frames: Option<u64>,
    pub gapless_trim: Option<GaplessTrimSpec>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeStatus {
    Produced { frames: usize },
    Pending,
    EndOfStream,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeekResult {
    pub actual_frame: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecoderSeekStatus {
    Pending,
    Complete(SeekResult),
}

pub trait DecoderStage: Send {
    fn open(
        &mut self,
        source: Box<dyn EncodedSource>,
        hints: &MediaHints,
    ) -> Result<DecodedStreamInfo, DecodeError>;
    fn decode(&mut self, output: &mut AudioBlock) -> Result<DecodeStatus, DecodeError>;
    fn start_seek(&mut self, target_frame: u64) -> Result<DecoderSeekStatus, DecodeError>;
    fn continue_seek(&mut self) -> Result<DecoderSeekStatus, DecodeError>;
    fn reset(&mut self);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecoderDescriptor {
    pub id: StageId,
    pub priority: i32,
    pub extensions: Vec<String>,
    pub mime_types: Vec<String>,
}

pub trait DecoderFactory: Send + Sync {
    fn descriptor(&self) -> &DecoderDescriptor;
    fn create(&self) -> Result<Box<dyn DecoderStage>, FactoryError>;
}
