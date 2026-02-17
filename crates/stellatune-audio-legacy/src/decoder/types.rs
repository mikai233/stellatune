#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrackSpec {
    pub sample_rate: u32,
    pub channels: u16,
}
