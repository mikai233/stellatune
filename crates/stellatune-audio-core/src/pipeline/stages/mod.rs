pub mod decoder;
pub mod sink;
pub mod source;
pub mod transform;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageStatus {
    Ok,
    Eof,
    Fatal,
}

impl StageStatus {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Eof | Self::Fatal)
    }
}
