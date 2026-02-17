use std::collections::VecDeque;

const GAPLESS_ENTRY_DECLICK_MS: usize = 2;

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct GaplessTrimSpec {
    pub(super) head_frames: u32,
    pub(super) tail_frames: u32,
}

#[derive(Debug, Default)]
pub(crate) struct GaplessTrimState {
    initial_head_samples: usize,
    head_samples_remaining: usize,
    tail_hold_samples: usize,
    tail_buffer: VecDeque<f32>,
    pending_output: VecDeque<f32>,
    eof_reached: bool,
    channels: usize,
    entry_ramp_total_frames: usize,
    entry_ramp_applied_frames: usize,
    entry_ramp_active: bool,
}

impl GaplessTrimState {
    pub(super) fn new(spec: GaplessTrimSpec, channels: usize, sample_rate: u32) -> Self {
        let channels = channels.max(1);
        let initial_head_samples = (spec.head_frames as usize).saturating_mul(channels);
        let tail_hold_samples = (spec.tail_frames as usize).saturating_mul(channels);
        let entry_ramp_total_frames = ((sample_rate.max(1) as usize) * GAPLESS_ENTRY_DECLICK_MS)
            .saturating_div(1000)
            .max(1);
        let entry_ramp_active = initial_head_samples > 0;
        Self {
            initial_head_samples,
            head_samples_remaining: initial_head_samples,
            tail_hold_samples,
            tail_buffer: VecDeque::new(),
            pending_output: VecDeque::new(),
            eof_reached: false,
            channels,
            entry_ramp_total_frames,
            entry_ramp_applied_frames: 0,
            entry_ramp_active,
        }
    }

    pub(super) fn reset_for_seek(&mut self, position_ms: u64) {
        self.pending_output.clear();
        self.tail_buffer.clear();
        self.eof_reached = false;
        self.entry_ramp_applied_frames = 0;
        self.head_samples_remaining = if position_ms == 0 {
            self.entry_ramp_active = self.initial_head_samples > 0;
            self.initial_head_samples
        } else {
            self.entry_ramp_active = false;
            0
        };
    }

    fn apply_entry_ramp_in_place(&mut self, samples: &mut [f32]) {
        if !self.entry_ramp_active || samples.is_empty() {
            return;
        }
        let channels = self.channels.max(1);
        let frames = samples.len() / channels;
        if frames == 0 {
            return;
        }
        let remaining = self
            .entry_ramp_total_frames
            .saturating_sub(self.entry_ramp_applied_frames);
        if remaining == 0 {
            self.entry_ramp_active = false;
            return;
        }
        let apply_frames = remaining.min(frames);
        for frame in 0..apply_frames {
            let progress_frame = self.entry_ramp_applied_frames + frame + 1;
            let t = (progress_frame as f32 / self.entry_ramp_total_frames as f32).clamp(0.0, 1.0);
            let gain = t.sqrt();
            let base = frame * channels;
            for ch in 0..channels {
                samples[base + ch] *= gain;
            }
        }
        self.entry_ramp_applied_frames =
            self.entry_ramp_applied_frames.saturating_add(apply_frames);
        if self.entry_ramp_applied_frames >= self.entry_ramp_total_frames {
            self.entry_ramp_active = false;
        }
    }

    pub(super) fn push_decoded_samples(&mut self, mut samples: Vec<f32>) {
        if self.head_samples_remaining > 0 {
            let trim = self.head_samples_remaining.min(samples.len());
            if trim == samples.len() {
                samples.clear();
            } else if trim > 0 {
                samples = samples.split_off(trim);
            }
            self.head_samples_remaining = self.head_samples_remaining.saturating_sub(trim);
        }
        if samples.is_empty() {
            return;
        }
        self.apply_entry_ramp_in_place(&mut samples);

        if self.tail_hold_samples == 0 {
            self.pending_output.extend(samples);
            return;
        }

        self.tail_buffer.extend(samples);
        let releasable = self
            .tail_buffer
            .len()
            .saturating_sub(self.tail_hold_samples);
        if releasable > 0 {
            self.pending_output
                .extend(self.tail_buffer.drain(..releasable));
        }
    }

    pub(super) fn on_eof(&mut self) {
        self.eof_reached = true;
        self.tail_buffer.clear();
    }

    pub(super) fn pending_output_is_empty(&self) -> bool {
        self.pending_output.is_empty()
    }

    pub(super) fn eof_reached(&self) -> bool {
        self.eof_reached
    }

    pub(super) fn pending_output_len(&self) -> usize {
        self.pending_output.len()
    }

    pub(super) fn drain_pending(&mut self, count: usize) -> Vec<f32> {
        let take = count.min(self.pending_output.len());
        let mut out = Vec::with_capacity(take);
        out.extend(self.pending_output.drain(..take));
        out
    }
}
