use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SinkLatencyConfig {
    pub target_latency_ms: u32,
    pub block_frames: u32,
    pub min_queue_blocks: usize,
    pub max_queue_blocks: usize,
}

impl SinkLatencyConfig {
    pub fn queue_capacity(self, sample_rate: u32) -> usize {
        let min_blocks = self.min_queue_blocks.max(1);
        let max_blocks = self.max_queue_blocks.max(min_blocks);
        let block_frames = self.block_frames.max(1) as u64;
        let target_frames = (sample_rate as u64 * self.target_latency_ms as u64).div_ceil(1000);
        let mut blocks = target_frames.div_ceil(block_frames) as usize;
        if blocks == 0 {
            blocks = 1;
        }
        blocks.clamp(min_blocks, max_blocks)
    }
}

impl Default for SinkLatencyConfig {
    fn default() -> Self {
        Self {
            target_latency_ms: 12,
            block_frames: 128,
            min_queue_blocks: 1,
            max_queue_blocks: 20,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SinkRecoveryConfig {
    pub max_attempts: u32,
    pub initial_backoff: Duration,
    pub max_backoff: Duration,
}

impl Default for SinkRecoveryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 6,
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(2),
        }
    }
}
