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
