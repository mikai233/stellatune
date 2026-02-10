use super::decoder;
use crate::engine::event_hub::EventHub;
use crate::engine::messages::{DecodeCtrl, InternalMsg, OutputSinkTx};
use crate::ring_buffer::RingBufferProducer;
use crossbeam_channel::{Receiver, Sender};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use stellatune_mixer::ChannelMixer;

use super::dsp::ActiveDspNode;

pub(crate) struct DecodeContext<'a> {
    pub(crate) path: &'a str,
    pub(crate) playing: &'a mut bool,
    pub(crate) last_emit: &'a mut Instant,
    pub(crate) dsp_chain: &'a mut Vec<ActiveDspNode>,
    pub(crate) decoder: &'a mut Box<decoder::EngineDecoder>,
    pub(crate) resampler: &'a mut Option<rubato::Async<f32>>,
    pub(crate) producer: &'a Arc<Mutex<RingBufferProducer<f32>>>,
    pub(crate) decode_pending: &'a mut Vec<f32>,
    pub(crate) out_pending: &'a mut Vec<f32>,
    pub(crate) frames_written: &'a mut u64,
    pub(crate) base_ms: &'a mut i64,
    pub(crate) lfe_mode: &'a mut stellatune_mixer::LfeMode,
    pub(crate) channel_mixer: &'a mut ChannelMixer,
    pub(crate) pending_seek: &'a mut Option<i64>,

    pub(crate) in_channels: usize,
    pub(crate) out_channels: usize,
    pub(crate) spec_sample_rate: u32,
    pub(crate) target_sample_rate: u32,

    pub(crate) output_enabled: &'a AtomicBool,
    pub(crate) output_sink_tx: &'a mut Option<OutputSinkTx>,
    pub(crate) output_sink_chunk_frames: &'a mut u32,
    pub(crate) output_sink_only: bool,
    pub(crate) events: &'a EventHub,
    pub(crate) ctrl_rx: &'a Receiver<DecodeCtrl>,
    pub(crate) internal_tx: &'a Sender<InternalMsg>,
}
