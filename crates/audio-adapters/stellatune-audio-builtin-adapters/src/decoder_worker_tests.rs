use std::{
    io::{self, Cursor, Read, Seek, SeekFrom},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
use stellatune_audio_core::{
    decoder::{DecodeStatus, DecoderFactory, DecoderSeekStatus},
    format::AudioBlock,
    source::{EncodedSource, MediaHints},
};

struct FragmentedSource {
    bytes: Cursor<Vec<u8>>,
    pending: bool,
    stalled: Arc<AtomicBool>,
    dropped: mpsc::Sender<()>,
}
impl Read for FragmentedSource {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.pending = !self.pending;
        if self.pending || self.stalled.load(Ordering::Acquire) {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        let length = output.len().min(31);
        self.bytes.read(&mut output[..length])
    }
}
impl Seek for FragmentedSource {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.bytes.seek(position)
    }
}
impl EncodedSource for FragmentedSource {
    fn byte_len(&self) -> Option<u64> {
        Some(self.bytes.get_ref().len() as u64)
    }
    fn is_seekable(&self) -> bool {
        true
    }
}
impl Drop for FragmentedSource {
    fn drop(&mut self) {
        let _ = self.dropped.send(());
    }
}

#[test]
fn fragmented_packets_preserve_every_sample_and_decoder_reset_cancels_pending_io() {
    let samples: Vec<i16> = (0..8000).map(|value| (value % 1000) as i16).collect();
    let mut wave = Vec::new();
    wave.extend_from_slice(b"RIFF");
    wave.extend_from_slice(&(36 + samples.len() as u32 * 2).to_le_bytes());
    wave.extend_from_slice(b"WAVEfmt ");
    wave.extend_from_slice(&16_u32.to_le_bytes());
    wave.extend_from_slice(&1_u16.to_le_bytes());
    wave.extend_from_slice(&1_u16.to_le_bytes());
    wave.extend_from_slice(&8000_u32.to_le_bytes());
    wave.extend_from_slice(&16000_u32.to_le_bytes());
    wave.extend_from_slice(&2_u16.to_le_bytes());
    wave.extend_from_slice(&16_u16.to_le_bytes());
    wave.extend_from_slice(b"data");
    wave.extend_from_slice(&(samples.len() as u32 * 2).to_le_bytes());
    for sample in &samples {
        wave.extend_from_slice(&sample.to_le_bytes());
    }
    let stalled = Arc::new(AtomicBool::new(false));
    let (dropped, receiver) = mpsc::channel();
    let mut decoder = crate::factories::SymphoniaDecoderFactory::new()
        .create()
        .unwrap();
    let info = decoder
        .open(
            Box::new(FragmentedSource {
                bytes: Cursor::new(wave),
                pending: false,
                stalled: stalled.clone(),
                dropped,
            }),
            &MediaHints {
                extension: Some("wav".into()),
                ..Default::default()
            },
        )
        .unwrap();
    let mut block = AudioBlock::new(info.format);
    let mut decoded = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        assert!(Instant::now() < deadline);
        let start = Instant::now();
        let status = decoder.decode(&mut block).unwrap();
        assert!(
            start.elapsed() < Duration::from_millis(100),
            "actor-facing decode must not wait for source bytes"
        );
        match status {
            DecodeStatus::Produced { .. } => decoded.extend_from_slice(&block.samples),
            DecodeStatus::Pending => std::thread::sleep(Duration::from_millis(2)),
            DecodeStatus::EndOfStream => break,
        }
    }
    assert_eq!(decoded.len(), samples.len());
    for (actual, expected) in decoded.iter().zip(samples) {
        assert!((*actual - f32::from(expected) / 32768.0).abs() < 1e-6);
    }
    let mut seek = decoder.start_seek(0).unwrap();
    while matches!(seek, DecoderSeekStatus::Pending) {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
        seek = decoder.continue_seek().unwrap();
    }
    stalled.store(true, Ordering::Release);
    let _ = decoder.decode(&mut block).unwrap();
    decoder.reset();
    receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("reset releases worker source even during a stalled read");
}

struct CountingDecoder {
    source: Option<Box<dyn EncodedSource>>,
    calls: mpsc::Sender<()>,
    frame: u64,
}

struct ReadyWake(mpsc::Sender<()>);
impl std::task::Wake for ReadyWake {
    fn wake(self: Arc<Self>) {
        let _ = self.0.send(());
    }
    fn wake_by_ref(self: &Arc<Self>) {
        let _ = self.0.send(());
    }
}
impl stellatune_audio_core::decoder::DecoderStage for CountingDecoder {
    fn open(
        &mut self,
        source: Box<dyn EncodedSource>,
        _: &MediaHints,
    ) -> Result<
        stellatune_audio_core::decoder::DecodedStreamInfo,
        stellatune_audio_core::error::DecodeError,
    > {
        self.source = Some(source);
        Ok(stellatune_audio_core::decoder::DecodedStreamInfo {
            format: stellatune_audio_core::format::PcmFormat {
                sample_rate: 8000,
                channel_layout: stellatune_audio_core::format::ChannelLayout::MONO,
            },
            duration_frames: None,
            gapless_trim: None,
        })
    }
    fn decode(
        &mut self,
        output: &mut AudioBlock,
    ) -> Result<DecodeStatus, stellatune_audio_core::error::DecodeError> {
        self.calls.send(()).unwrap();
        output.samples = (self.frame..self.frame + 8).map(|v| v as f32).collect();
        self.frame += 8;
        Ok(DecodeStatus::Produced { frames: 8 })
    }
    fn start_seek(
        &mut self,
        frame: u64,
    ) -> Result<DecoderSeekStatus, stellatune_audio_core::error::DecodeError> {
        self.frame = frame;
        Ok(DecoderSeekStatus::Complete(
            stellatune_audio_core::decoder::SeekResult {
                actual_frame: frame,
            },
        ))
    }
    fn continue_seek(
        &mut self,
    ) -> Result<DecoderSeekStatus, stellatune_audio_core::error::DecodeError> {
        unreachable!()
    }
    fn reset(&mut self) {
        self.source = None;
    }
}

#[test]
fn bounded_read_ahead_discards_old_pcm_on_seek_and_reset_unblocks_a_full_queue() {
    let (calls, decoded) = mpsc::channel();
    let (dropped, released) = mpsc::channel();
    let mut worker = crate::decoder_worker::DecoderWorker::default();
    worker.configure_buffering(stellatune_audio_core::buffering::BufferingConfig {
        decode_ahead_ms: 8, // 64 frames at 8 kHz, regardless of decoder block count.
        ..Default::default()
    });
    let info = worker
        .open(
            Box::new(CountingDecoder {
                source: None,
                calls,
                frame: 0,
            }),
            Box::new(FragmentedSource {
                bytes: Cursor::new(vec![]),
                pending: false,
                stalled: Arc::new(AtomicBool::new(false)),
                dropped,
            }),
            &MediaHints::default(),
        )
        .unwrap();
    let mut block = AudioBlock::new(info.format);
    let (ready, notifications) = mpsc::channel();
    worker.set_waker(std::task::Waker::from(Arc::new(ReadyWake(ready))));
    let first = worker.decode(&mut block).unwrap();
    notifications
        .recv_timeout(Duration::from_secs(1))
        .expect("PCM completion wakes the runtime");
    // Eight queued replies, one in-flight send, and possibly the first reply
    // already consumed by decode. A stalled consumer must bound read-ahead.
    let count = if matches!(first, DecodeStatus::Produced { .. }) {
        10
    } else {
        9
    };
    for _ in 0..count {
        decoded.recv_timeout(Duration::from_secs(1)).unwrap();
    }
    assert!(decoded.recv_timeout(Duration::from_millis(20)).is_err());
    while notifications.try_recv().is_ok() {}
    let mut status = worker.start_seek(1000).unwrap();
    notifications
        .recv_timeout(Duration::from_secs(1))
        .expect("seek completion wakes the runtime");
    let deadline = Instant::now() + Duration::from_secs(1);
    while matches!(status, DecoderSeekStatus::Pending) {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
        status = worker.continue_seek().unwrap();
    }
    loop {
        block.timeline.start_frame = 1000;
        block.timeline.epoch = 7;
        if matches!(
            worker.decode(&mut block).unwrap(),
            DecodeStatus::Produced { .. }
        ) {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert_eq!(
        block.samples,
        (1000..1008).map(|v| v as f32).collect::<Vec<_>>()
    );
    assert_eq!(block.timeline.start_frame, 1000);
    assert_eq!(block.timeline.epoch, 7);
    // Leave the reply queue full again before resetting.
    for _ in 0..10 {
        decoded.recv_timeout(Duration::from_secs(1)).unwrap();
    }
    worker.reset();
    released
        .recv_timeout(Duration::from_secs(1))
        .expect("reset releases a blocked producer");
}
