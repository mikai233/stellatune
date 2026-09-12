use super::*;
use std::io::Cursor;

fn wave(bits: u16, stored_bits: u16, float: bool) -> Vec<u8> {
    let mut b = b"RIFF".to_vec();
    b.extend(64u32.to_le_bytes());
    b.extend(b"WAVEfmt ");
    b.extend(40u32.to_le_bytes());
    b.extend(0xfffeu16.to_le_bytes());
    b.extend(2u16.to_le_bytes());
    b.extend(96000u32.to_le_bytes());
    b.extend((96000 * u32::from(stored_bits / 8) * 2).to_le_bytes());
    b.extend((stored_bits / 8 * 2).to_le_bytes());
    b.extend(stored_bits.to_le_bytes());
    b.extend(22u16.to_le_bytes());
    b.extend(bits.to_le_bytes());
    b.extend(3u32.to_le_bytes());
    b.extend(if float { 3u32 } else { 1u32 }.to_le_bytes());
    b.extend([0, 0, 16, 0, 128, 0, 0, 170, 0, 56, 155, 113]);
    b.extend(b"data");
    b.extend(4u32.to_le_bytes());
    b.extend([0; 4]);
    b
}
#[test]
fn pcm_uses_storage_width_instead_of_valid_bits() {
    for (bits, stored, float) in [
        (16, 16, false),
        (24, 24, false),
        (24, 32, false),
        (32, 32, true),
        (64, 64, true),
    ] {
        let result = probe(
            Cursor::new(wave(bits, stored, float)),
            Some("wav"),
            &AtomicBool::new(false),
        );
        assert_eq!(result.status, ProbeStatus::Ready);
        let p = result.properties.unwrap();
        assert_eq!(p.bits_per_sample, Some(bits.into()));
        assert_eq!(p.floating_point, float);
        assert_eq!(
            p.bitrate.unwrap(),
            BitrateInfo {
                bps: 96000 * 2 * u32::from(stored),
                kind: BitrateKind::Fixed,
                estimated: false,
                mode: Some(BitrateMode::Cbr)
            }
        );
        assert!(result.bytes_read < 100);
    }
}
#[test]
fn budget_counts_rereads_and_cancellation_is_sticky() {
    let cancel = AtomicBool::new(false);
    let mut r = BudgetReader::new(Cursor::new(vec![0; 1024]), &cancel);
    r.remaining = 1500;
    let mut b = [0; 1024];
    assert_eq!(r.read(&mut b).unwrap(), 1024);
    r.rewind().unwrap();
    assert_eq!(r.read(&mut b).unwrap(), 476);
    assert!(r.read(&mut b).is_err());
    assert_eq!(r.failure, Some(ProbeStatus::BudgetExceeded));
    let mut r = BudgetReader::new(Cursor::new(vec![0; 10]), &cancel);
    cancel.store(true, Ordering::Relaxed);
    assert!(r.seek(SeekFrom::End(0)).is_err());
    assert_eq!(r.failure, Some(ProbeStatus::Cancelled));
    let result = probe(Cursor::new(wave(16, 16, false)), None, &cancel);
    assert_eq!(result.status, ProbeStatus::Cancelled);
    assert_eq!(result.bytes_read, 0);
}
#[test]
fn expired_deadline_discards_properties() {
    let cancel = AtomicBool::new(false);
    let mut r = BudgetReader::new(Cursor::new(vec![0; 10]), &cancel);
    r.deadline = Instant::now() - Duration::from_millis(1);
    assert!(r.read(&mut [0; 1]).is_err());
    assert_eq!(r.failure, Some(ProbeStatus::BudgetExceeded));
}
#[test]
fn slice_is_zero_based_and_never_reads_container_bytes() {
    let payload = wave(24, 32, false);
    let expected = probe(Cursor::new(&payload), None, &AtomicBool::new(false)).properties;
    let mut bytes = vec![0x55; 91];
    bytes.extend(&payload);
    bytes.extend([0xAA; 37]);
    let mut slice = slice::AudioSlice::new(Cursor::new(bytes), 91, payload.len() as u64).unwrap();
    assert_eq!(
        slice.seek(SeekFrom::End(-4)).unwrap(),
        payload.len() as u64 - 4
    );
    let mut tail = Vec::new();
    slice.read_to_end(&mut tail).unwrap();
    assert_eq!(tail, [0; 4]);
    assert!(slice.seek(SeekFrom::Current(1)).is_err());
    assert!(slice.seek(SeekFrom::End(-10000)).is_err());
    slice.rewind().unwrap();
    assert_eq!(
        probe(slice, None, &AtomicBool::new(false)).properties,
        expected
    );
}
#[test]
fn scan_formats_exclude_video_and_unsupported_decoders() {
    for ext in [
        "MP1", "mp2", "mp3", "aac", "flac", "wave", "aiff", "aifc", "m4a", "m4b", "m4r", "alac",
        "ogg", "oga", "caf",
    ] {
        assert!(formats::auto_scan_extension(ext), "{ext}");
    }
    for ext in ["mp4", "mov", "3gp", "m4p", "ape", "opus", "wv"] {
        assert!(!formats::auto_scan_extension(ext), "{ext}");
    }
}
#[test]
fn mp3_audio_bitrate_keeps_estimation_and_no_invented_mode() {
    let bytes = include_bytes!("../../stellatune-library/tests/fixtures/metadata-id3.mp3");
    let result = probe(Cursor::new(bytes), Some("mp3"), &AtomicBool::new(false));
    let bitrate = result.properties.unwrap().bitrate.unwrap();
    // This very short Info-header fixture includes frame overhead in Lofty's
    // average (73 kbps despite a 64 kbps encoder setting). Do not claim exact CBR.
    assert_eq!(bitrate.bps, 73000);
    assert!(bitrate.estimated);
    assert_eq!(bitrate.mode, None);
}
#[test]
#[ignore = "Read-only user supplied audio sample"]
fn real_audio_properties() {
    let path = std::env::var("STELLATUNE_AUDIO_AUDIT").unwrap();
    let result = probe(
        std::fs::File::open(&path).unwrap(),
        None,
        &AtomicBool::new(false),
    );
    println!("{path}: {result:?}");
    assert_eq!(result.status, ProbeStatus::Ready);
    if let Ok(rate) = std::env::var("STELLATUNE_AUDIO_EXPECTED_RATE") {
        assert_eq!(
            result.properties.unwrap().bitrate.unwrap().bps,
            rate.parse::<u32>().unwrap()
        );
    }
}

#[test]
fn compressed_and_pcm_format_fixtures() {
    for (name, codec, estimated, fixed) in [
        ("cbr.mp3", "mp3", true, false),
        ("no-info.mp3", "mp3", true, false),
        ("layer2.mp2", "mp2", true, false),
        ("vbr.mp3", "mp3", true, false),
        ("adts.aac", "aac", true, false),
        ("aac.m4a", "aac", false, false),
        ("alac.m4a", "alac", false, false),
        ("tone.flac", "flac", false, false),
        ("vorbis.ogg", "vorbis", false, false),
        ("opus.ogg", "opus", false, false),
        ("float.caf", "pcm_float", false, true),
        ("integer.aiff", "pcm", false, true),
    ] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name);
        let result = probe(
            std::fs::File::open(path).unwrap(),
            None,
            &AtomicBool::new(false),
        );
        println!("{name}: {result:?}");
        assert_eq!(result.status, ProbeStatus::Ready, "{name}");
        let p = result.properties.unwrap();
        assert_eq!(p.codec.as_deref(), Some(codec), "{name}");
        assert_eq!(p.sample_rate, Some(48000), "{name}");
        assert_eq!(p.channels, Some(2), "{name}");
        let b = p.bitrate.unwrap();
        assert_eq!(b.estimated, estimated, "{name}");
        assert_eq!(
            b.kind,
            if fixed {
                BitrateKind::Fixed
            } else {
                BitrateKind::Average
            },
            "{name}"
        );
        if fixed {
            assert_eq!(
                b.bps,
                48000 * 2 * if name.ends_with("caf") { 32 } else { 24 }
            );
        }
        assert!(result.bytes_read <= READ_LIMIT);
    }
}

#[test]
fn huge_id3_tag_is_skipped_and_does_not_change_audio_bitrate() {
    let bytes = include_bytes!("../tests/fixtures/cbr.mp3");
    let tag_len = bytes[6..10]
        .iter()
        .fold(0usize, |n, b| (n << 7) | usize::from(*b));
    let mut big = b"ID3\x04\0\0\x05\0\0\0".to_vec(); // 10 MiB tag: larger than read budget.
    big.resize(10 + 10 * 1024 * 1024, 0);
    big.extend(&bytes[10 + tag_len..]);
    let result = probe(Cursor::new(big), Some("mp3"), &AtomicBool::new(false));
    assert_eq!(result.status, ProbeStatus::Ready);
    let original = probe(Cursor::new(bytes), Some("mp3"), &AtomicBool::new(false));
    assert_eq!(
        result.properties.unwrap().bitrate,
        original.properties.unwrap().bitrate
    );
    assert!(result.bytes_read < 4096);
}

#[test]
fn concurrent_probes_are_limited_to_two_readers() {
    use std::sync::atomic::AtomicUsize;
    static READING: AtomicUsize = AtomicUsize::new(0);
    static MAX: AtomicUsize = AtomicUsize::new(0);
    struct Slow(Cursor<Vec<u8>>);
    impl Read for Slow {
        fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
            let n = READING.fetch_add(1, Ordering::SeqCst) + 1;
            MAX.fetch_max(n, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(2));
            let result = self.0.read(b);
            READING.fetch_sub(1, Ordering::SeqCst);
            result
        }
    }
    impl Seek for Slow {
        fn seek(&mut self, p: SeekFrom) -> io::Result<u64> {
            self.0.seek(p)
        }
    }
    std::thread::scope(|scope| {
        for _ in 0..8 {
            scope.spawn(|| {
                assert_eq!(
                    probe(
                        Slow(Cursor::new(wave(16, 16, false))),
                        None,
                        &AtomicBool::new(false)
                    )
                    .status,
                    ProbeStatus::Ready
                )
            });
        }
    });
    assert!(MAX.load(Ordering::SeqCst) <= 2);
}
