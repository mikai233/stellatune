//! Audio-processing quality and channel-routing choices.

/// The policy applied when an input LFE channel has no matching output channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LfeMode {
    /// Drops LFE content instead of routing it to full-range speakers.
    #[default]
    Mute,
    /// Mixes LFE content into the front speakers.
    MixToFront,
}

/// A resampling quality and CPU-cost preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResampleQuality {
    /// Prioritizes throughput and low CPU usage.
    Fast,
    /// Balances conversion quality and CPU usage.
    Balanced,
    /// Prioritizes conversion quality while retaining practical runtime cost.
    #[default]
    High,
    /// Uses the highest available quality at the greatest CPU cost.
    Ultra,
}
