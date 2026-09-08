#![cfg(feature = "test-host")]
use std::{
    path::PathBuf,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};
use stellatune_audio_asio_adapter::{AsioConfig, AsioSinkFactory};
use stellatune_audio_core::{
    buffering::{LatencyProfile, frames_for_ms},
    format::{AudioBlock, ChannelLayout, PcmFormat},
    sink::{SinkFactory, SinkWriteState},
};

fn factory(rate: u32, device: &str) -> Arc<AsioSinkFactory> {
    AsioSinkFactory::discover(
        PathBuf::from(env!("CARGO_BIN_EXE_fake-asio-host")),
        device.into(),
        AsioConfig {
            sample_rate: Some(rate),
            ..Default::default()
        },
        1,
    )
    .unwrap()
}
fn block(format: PcmFormat, frames: usize) -> AudioBlock {
    let mut block = AudioBlock::new(format);
    block.samples = [0.25, -0.25].repeat(frames);
    block
}
#[test]
fn pcm_backpressure_pause_reset_and_drain_work_across_processes() {
    for rate in [8000, 48000, 192000] {
        let route = factory(rate, "test");
        let format = route.format();
        let buffering = LatencyProfile::Low.buffering();
        let mut sink = route.create().unwrap();
        sink.configure_buffering(buffering);
        sink.open(format).unwrap();
        let frames = frames_for_ms(format, buffering.device_ms);
        let input = block(format, frames);
        assert_eq!(sink.write(&input).unwrap().consumed_frames, frames);
        assert_eq!(
            sink.write(&input).unwrap().state,
            SinkWriteState::WouldBlock
        );
        thread::sleep(Duration::from_millis(25));
        assert_eq!(sink.clock_snapshot().consumed_frames, 0);
        sink.resume().unwrap();
        sink.drain().unwrap();
        assert_eq!(sink.clock_snapshot().consumed_frames, frames as u64);
        sink.pause().unwrap();
        sink.write(&input).unwrap();
        let epoch = sink.clock_snapshot().epoch;
        sink.discard().unwrap();
        assert!(sink.clock_snapshot().epoch > epoch);
        assert_eq!(sink.clock_snapshot().buffered_frames, 0);
        assert_eq!(sink.clock_snapshot().consumed_frames, 0);
        for _ in 0..12 {
            // exercise wrapping and successive reset clock bases
            sink.write(&input).unwrap();
            sink.resume().unwrap();
            sink.drain().unwrap();
            sink.pause().unwrap();
        }
        assert_eq!(sink.clock_snapshot().consumed_frames, frames as u64 * 12);
        route.revoke();
        assert!(sink.write(&input).is_err());
        assert!(route.create().is_err());
        sink.close();
    }
}
#[test]
fn failed_open_crash_and_unresponsive_driver_can_be_reopened() {
    for device in ["fail-open", "crash", "hang"] {
        let route = factory(48000, device);
        let mut sink = route.create().unwrap();
        let started = Instant::now();
        if device == "fail-open" {
            assert!(sink.open(route.format()).is_err());
        } else {
            sink.open(route.format()).unwrap();
            while sink.write(&block(route.format(), 1)).is_ok() {
                assert!(started.elapsed() < Duration::from_secs(6));
                thread::sleep(Duration::from_millis(10));
            }
        }
        sink.close();
        assert!(started.elapsed() < Duration::from_secs(7));
        let good = factory(48000, "test");
        let mut reopened = good.create().unwrap();
        reopened.open(good.format()).unwrap();
        reopened.close();
    }
}
#[test]
fn native_format_is_explicit_and_rejects_unsupported_rate() {
    let route = factory(192000, "test");
    assert_eq!(
        route
            .preferred_format(PcmFormat {
                sample_rate: 44100,
                channel_layout: ChannelLayout::MONO
            })
            .unwrap(),
        route.format()
    );
    assert!(
        AsioSinkFactory::discover(
            PathBuf::from(env!("CARGO_BIN_EXE_fake-asio-host")),
            "test".into(),
            AsioConfig {
                sample_rate: Some(12345),
                ..Default::default()
            },
            1
        )
        .is_err()
    );
}

#[test]
fn track_rate_policy_negotiates_and_transports_each_supported_rate() {
    let route = AsioSinkFactory::discover(
        PathBuf::from(env!("CARGO_BIN_EXE_fake-asio-host")),
        "test".into(),
        AsioConfig::default(),
        2,
    )
    .unwrap();
    let matched = route.with_match_track_sample_rate(true);
    for rate in [8000, 48000, 192000] {
        let input = PcmFormat {
            sample_rate: rate,
            channel_layout: ChannelLayout::STEREO,
        };
        assert_eq!(route.preferred_format(input).unwrap().sample_rate, 48000);
        let output = matched.preferred_format(input).unwrap();
        assert_eq!(output, input);
        assert_eq!(matched.compatibility_key(output).unwrap().sample_rate, rate);
        let mut sink = matched.create().unwrap();
        sink.open(output).unwrap();
        let audio = block(output, 80);
        assert_eq!(sink.write(&audio).unwrap().consumed_frames, 80);
        sink.resume().unwrap();
        sink.drain().unwrap();
        assert_eq!(sink.clock_snapshot().consumed_frames, 80);
        let wrong = PcmFormat {
            sample_rate: if rate == 48000 { 8000 } else { 48000 },
            ..output
        };
        assert!(sink.write(&block(wrong, 1)).is_err());
        sink.close();
    }
    let unsupported = PcmFormat {
        sample_rate: 12345,
        channel_layout: ChannelLayout::MONO,
    };
    assert_eq!(
        matched.preferred_format(unsupported).unwrap(),
        route.format()
    );
    let fixed = factory(192000, "test").with_match_track_sample_rate(true);
    assert_eq!(
        fixed.preferred_format(route.format()).unwrap().sample_rate,
        192000
    );
}
