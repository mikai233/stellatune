use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use stellatune_audio_core::{
    AudioBlock, ChannelLayout, MediaTime, OutputCompatibilityKey, PcmFormat, PlaybackControlError,
    PlaybackItemId, SpeakerPosition, StageId, TransformPlacement,
};
use tokio::time::{Duration, timeout};

use crate::planner::{
    CrossfadeCurve, CrossfadeFallback, GainCurve, StageRegistrySnapshot, TransitionPolicy,
};
use crate::playback::control::SwitchOptions;
use crate::playback::event::{PlaybackEvent, PlaybackState};
use crate::playback::normalizer::{ChannelMixer, PcmNormalizer, SQRT_HALF};
use crate::playback::runtime::{PlaybackRuntime, PlaybackRuntimeConfig};
use crate::playback::sink_worker::OutputGainEnvelope;

use super::support::*;

#[tokio::test]
async fn sink_disconnect_recovers_from_consumed_checkpoint() {
    let samples = Arc::new(Mutex::new(Vec::new()));
    let creates = Arc::new(AtomicUsize::new(0));
    let registry = StageRegistrySnapshot {
        decoders: vec![Arc::new(TestDecoderFactory::new())],
        transforms: Vec::new(),
        sink: Arc::new(RecoveringSinkFactory {
            id: StageId::new("test.recovering-sink").unwrap(),
            samples: Arc::clone(&samples),
            creates: Arc::clone(&creates),
        }),
    };
    let mut config = PlaybackRuntimeConfig::new(registry);
    config.block_frames = 10;
    config.pcm_ring_blocks = 2;
    config.policies.seek_fade_frames = 0;
    config.policies.recovery_backoff_ms = 0;
    let runtime = PlaybackRuntime::start(config).unwrap();
    let controller = runtime.controller();
    let mut events = controller.subscribe_events();

    controller
        .switch(item(1, 40, 100), SwitchOptions::default())
        .await
        .unwrap();

    let saw_recovering = timeout(Duration::from_secs(3), async {
        let mut recovering = false;
        loop {
            match events.recv().await.unwrap() {
                PlaybackEvent::StateChanged(PlaybackState::Recovering) => recovering = true,
                PlaybackEvent::PlaybackEnded { .. } => break recovering,
                _ => {},
            }
        }
    })
    .await
    .expect("playback should recover and finish");

    assert!(saw_recovering);
    assert!(creates.load(Ordering::SeqCst) >= 2);
    assert_eq!(samples.lock().unwrap().len(), 40);
    runtime.shutdown().await.unwrap();
}

#[test]
fn output_gain_envelope_advances_by_audio_frames() {
    let mut envelope = OutputGainEnvelope::new(1.0);
    envelope.schedule(0.0, 4);
    let mut block = AudioBlock::new(PcmFormat {
        sample_rate: 1_000,
        channel_layout: ChannelLayout::STEREO,
    });
    block.samples = vec![1.0; 10];
    envelope.apply(&mut block);
    assert_eq!(
        block.samples,
        vec![0.75, 0.75, 0.5, 0.5, 0.25, 0.25, 0.0, 0.0, 0.0, 0.0]
    );
}

#[test]
fn channel_mixer_downmixes_5_1_by_position_and_drops_lfe() {
    let mixer = ChannelMixer::new(ChannelLayout::SURROUND_5_1_SIDE, ChannelLayout::STEREO).unwrap();
    let positions = ChannelLayout::SURROUND_5_1_SIDE
        .positions()
        .collect::<Vec<_>>();
    let render_position = |position| {
        let mut input = vec![0.0; positions.len()];
        input[positions.iter().position(|item| *item == position).unwrap()] = 1.0;
        mixer.process(&input)
    };

    assert_eq!(render_position(SpeakerPosition::Lfe), vec![0.0, 0.0]);
    assert_eq!(render_position(SpeakerPosition::SideLeft)[1], 0.0);
    assert_eq!(render_position(SpeakerPosition::SideRight)[0], 0.0);
    assert!(render_position(SpeakerPosition::FrontCenter)[0] > 0.0);

    let full_scale = mixer.process(&vec![1.0; positions.len()]);
    assert!(full_scale.iter().all(|sample| sample.abs() <= 1.0));
}

#[test]
fn channel_mixer_conservatively_expands_stereo_to_7_1() {
    let mixer = ChannelMixer::new(ChannelLayout::STEREO, ChannelLayout::SURROUND_7_1).unwrap();
    let output = mixer.process(&[0.25, -0.5]);

    assert_eq!(output.len(), 8);
    assert_eq!(output[0], 0.25);
    assert_eq!(output[1], -0.5);
    assert!(output[2..].iter().all(|sample| *sample == 0.0));
}

#[test]
fn channel_mixer_folds_height_channels_into_matching_bed_channels() {
    let mixer =
        ChannelMixer::new(ChannelLayout::SURROUND_7_1_4, ChannelLayout::SURROUND_7_1).unwrap();
    let source_positions = ChannelLayout::SURROUND_7_1_4
        .positions()
        .collect::<Vec<_>>();
    let mut input = vec![0.0; source_positions.len()];
    input[source_positions
        .iter()
        .position(|position| *position == SpeakerPosition::TopRearLeft)
        .unwrap()] = 1.0;
    let output = mixer.process(&input);

    let rear_left = ChannelLayout::SURROUND_7_1
        .index_of(SpeakerPosition::RearLeft)
        .unwrap();
    assert!(output[rear_left] > 0.0);
    assert_eq!(output.iter().filter(|sample| **sample != 0.0).count(), 1);
}

#[test]
fn channel_mixer_uses_constant_power_mono_to_stereo_routing() {
    let mixer = ChannelMixer::new(ChannelLayout::MONO, ChannelLayout::STEREO).unwrap();
    assert_eq!(mixer.process(&[1.0]), vec![SQRT_HALF, SQRT_HALF]);

    let mixer = ChannelMixer::new(ChannelLayout::STEREO, ChannelLayout::MONO).unwrap();
    assert_eq!(mixer.process(&[1.0, -0.5]), vec![0.25]);
}

#[test]
fn channel_mixer_merges_rear_channels_into_5_1_side_layout() {
    let mixer = ChannelMixer::new(
        ChannelLayout::SURROUND_7_1,
        ChannelLayout::SURROUND_5_1_SIDE,
    )
    .unwrap();
    let source_positions = ChannelLayout::SURROUND_7_1.positions().collect::<Vec<_>>();
    let mut input = vec![0.0; source_positions.len()];
    input[source_positions
        .iter()
        .position(|position| *position == SpeakerPosition::RearLeft)
        .unwrap()] = 1.0;
    let output = mixer.process(&input);
    let side_left = ChannelLayout::SURROUND_5_1_SIDE
        .index_of(SpeakerPosition::SideLeft)
        .unwrap();

    assert!(output[side_left] > 0.0);
    assert_eq!(output.iter().filter(|sample| **sample != 0.0).count(), 1);
}

#[test]
fn output_compatibility_key_distinguishes_equal_channel_counts() {
    let side = OutputCompatibilityKey {
        backend_id: "test".to_owned(),
        device_id: None,
        sample_rate: 48_000,
        channel_layout: ChannelLayout::SURROUND_5_1_SIDE,
        route_revision: 0,
    };
    let rear = OutputCompatibilityKey {
        channel_layout: ChannelLayout::SURROUND_5_1_REAR,
        ..side.clone()
    };

    assert_ne!(side, rear);
}

#[test]
fn normalizer_trims_startup_delay_and_drains_exact_resampled_duration() {
    let source = PcmFormat {
        sample_rate: 44_100,
        channel_layout: ChannelLayout::MONO,
    };
    let target = PcmFormat {
        sample_rate: 48_000,
        channel_layout: ChannelLayout::STEREO,
    };
    let input_frames = 1_500_usize;
    let expected_frames = ((input_frames as f64 * target.sample_rate as f64
        / source.sample_rate as f64)
        .ceil()) as usize;
    let mut normalizer = PcmNormalizer::new(source, target).unwrap();
    let mut block = AudioBlock::new(source);
    block.samples = vec![1.0; input_frames];
    normalizer.process(&mut block).unwrap();
    let mut rendered = block.samples;

    loop {
        let mut tail = AudioBlock::new(target);
        if !normalizer.drain(&mut tail).unwrap() {
            break;
        }
        rendered.extend(tail.samples);
    }

    assert_eq!(
        rendered.len(),
        expected_frames * usize::from(target.channel_layout.channel_count())
    );
    assert!(rendered.iter().any(|sample| sample.abs() > 0.5));
}

#[test]
fn normalizer_preserves_a_stereo_sine_without_noise_spikes() {
    let source = PcmFormat {
        sample_rate: 44_100,
        channel_layout: ChannelLayout::STEREO,
    };
    let target = PcmFormat {
        sample_rate: 48_000,
        channel_layout: ChannelLayout::STEREO,
    };
    let input_frames = 44_100_usize;
    let mut input = Vec::with_capacity(input_frames * 2);
    for frame in 0..input_frames {
        let phase = std::f32::consts::TAU * 440.0 * frame as f32 / source.sample_rate as f32;
        input.extend_from_slice(&[phase.sin() * 0.5, phase.cos() * 0.25]);
    }

    let mut normalizer = PcmNormalizer::new(source, target).unwrap();
    let mut rendered = Vec::new();
    for samples in input.chunks(1024 * usize::from(source.channel_layout.channel_count())) {
        let mut block = AudioBlock::new(source);
        block.samples.extend_from_slice(samples);
        normalizer.process(&mut block).unwrap();
        rendered.extend(block.samples);
    }
    loop {
        let mut tail = AudioBlock::new(target);
        if !normalizer.drain(&mut tail).unwrap() {
            break;
        }
        rendered.extend(tail.samples);
    }

    assert!(rendered.iter().all(|sample| sample.is_finite()));
    assert!(rendered.iter().all(|sample| sample.abs() <= 0.51));
    let stable_samples = &rendered[..rendered
        .len()
        .saturating_sub(256 * usize::from(target.channel_layout.channel_count()))];
    for channel in 0..usize::from(target.channel_layout.channel_count()) {
        let (max_step_index, max_step) = stable_samples
            .chunks_exact(usize::from(target.channel_layout.channel_count()))
            .map(|frame| frame[channel])
            .zip(
                stable_samples
                    .chunks_exact(usize::from(target.channel_layout.channel_count()))
                    .skip(1)
                    .map(|frame| frame[channel]),
            )
            .map(|(left, right)| (right - left).abs())
            .enumerate()
            .max_by(|left, right| left.1.total_cmp(&right.1))
            .unwrap();
        assert!(
            max_step < 0.08,
            "channel {channel} has a {max_step} noise spike at frame {max_step_index}"
        );
    }
}

#[tokio::test]
async fn output_gain_set_before_switch_is_applied_by_sink_worker() {
    let samples = Arc::new(Mutex::new(Vec::new()));
    let runtime = runtime(TransitionPolicy::Gapless, Arc::clone(&samples));
    let controller = runtime.controller();
    controller
        .set_output_gain(0.5, MediaTime::ZERO)
        .await
        .unwrap();
    let mut events = controller.subscribe_events();
    controller
        .switch(item(1, 20, 100), SwitchOptions::default())
        .await
        .unwrap();
    wait_for_end(&mut events).await;
    let rendered = samples.lock().unwrap().clone();
    assert_eq!(rendered, vec![0.5; 20]);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn preferred_sink_format_normalizes_pcm_before_opening_output() {
    let target = PcmFormat {
        sample_rate: 200,
        channel_layout: ChannelLayout::STEREO,
    };
    let formats = Arc::new(Mutex::new(Vec::new()));
    let samples = Arc::new(Mutex::new(Vec::new()));
    let registry = StageRegistrySnapshot {
        decoders: vec![Arc::new(TestDecoderFactory::new())],
        transforms: Vec::new(),
        sink: Arc::new(FormatAdaptingSinkFactory {
            id: StageId::new("test.format-adapting-sink").unwrap(),
            target,
            formats: Arc::clone(&formats),
            samples: Arc::clone(&samples),
        }),
    };
    let mut config = PlaybackRuntimeConfig::new(registry);
    config.block_frames = 10;
    let runtime = PlaybackRuntime::start(config).unwrap();
    let controller = runtime.controller();
    let mut events = controller.subscribe_events();

    controller
        .switch(item(1, 20, 100), SwitchOptions::default())
        .await
        .unwrap();
    wait_for_end(&mut events).await;

    assert_eq!(formats.lock().unwrap().as_slice(), &[target]);
    assert_eq!(
        samples.lock().unwrap().len(),
        40 * usize::from(target.channel_layout.channel_count())
    );
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn rebuilding_output_while_idle_is_a_noop() {
    let samples = Arc::new(Mutex::new(Vec::new()));
    let runtime = runtime(TransitionPolicy::Gapless, samples);

    runtime.controller().rebuild_output().await.unwrap();

    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn controller_clones_do_not_own_runtime_and_stop_is_not_shutdown() {
    let samples = Arc::new(Mutex::new(Vec::new()));
    let runtime = runtime(TransitionPolicy::Gapless, Arc::clone(&samples));
    let controller = runtime.controller();
    let disposable_clone = controller.clone();
    drop(disposable_clone);

    controller
        .switch(
            item(1, 20, 100),
            SwitchOptions {
                autoplay: false,
                ..SwitchOptions::default()
            },
        )
        .await
        .unwrap();
    controller.stop().await.unwrap();

    let mut events = controller.subscribe_events();
    controller
        .switch(item(2, 20, 50), SwitchOptions::default())
        .await
        .unwrap();
    wait_for_end(&mut events).await;
    assert_eq!(samples.lock().unwrap().as_slice(), &[0.5; 20]);

    runtime.shutdown().await.unwrap();
    assert!(matches!(
        controller.snapshot().await,
        Err(PlaybackControlError::Closed)
    ));
}

#[tokio::test]
async fn pause_and_seek_preempt_a_sink_that_keeps_returning_would_block() {
    let allow_writes = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let registry = StageRegistrySnapshot {
        decoders: vec![Arc::new(TestDecoderFactory::new())],
        transforms: Vec::new(),
        sink: Arc::new(StalledSinkFactory {
            id: StageId::new("test.stalled-sink").unwrap(),
            allow_writes: Arc::clone(&allow_writes),
        }),
    };
    let mut config = PlaybackRuntimeConfig::new(registry);
    config.block_frames = 10;
    config.pcm_ring_blocks = 2;
    let runtime = PlaybackRuntime::start(config).unwrap();
    let controller = runtime.controller();
    let mut events = controller.subscribe_events();
    controller
        .switch(item(1, 100, 100), SwitchOptions::default())
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;

    timeout(Duration::from_millis(200), controller.pause())
        .await
        .expect("pause must not wait for the stalled partial write")
        .unwrap();
    timeout(
        Duration::from_millis(200),
        controller.seek(MediaTime::from_millis(500)),
    )
    .await
    .expect("seek discard must preempt the stalled partial write")
    .unwrap();
    assert_eq!(
        controller.snapshot().await.unwrap().consumed_position,
        MediaTime::from_millis(500)
    );

    allow_writes.store(true, Ordering::SeqCst);
    controller.play().await.unwrap();
    wait_for_end(&mut events).await;
    runtime.shutdown().await.unwrap();
}

async fn assert_buffered_tail_is_drained(placement: TransformPlacement) {
    let samples = Arc::new(Mutex::new(Vec::new()));
    let registry = StageRegistrySnapshot {
        decoders: vec![Arc::new(TestDecoderFactory::new())],
        transforms: vec![Arc::new(BufferingTailTransformFactory::new(placement))],
        sink: Arc::new(TestSinkFactory {
            id: StageId::new("test.sink").unwrap(),
            samples: Arc::clone(&samples),
        }),
    };
    let mut config = PlaybackRuntimeConfig::new(registry);
    config.block_frames = 10;
    let runtime = PlaybackRuntime::start(config).unwrap();
    let controller = runtime.controller();
    let mut events = controller.subscribe_events();
    controller
        .switch(item(1, 10, 80), SwitchOptions::default())
        .await
        .unwrap();
    wait_for_end(&mut events).await;
    assert_eq!(samples.lock().unwrap().as_slice(), &[0.8; 10]);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn buffered_pre_mix_tail_is_drained_before_end() {
    assert_buffered_tail_is_drained(TransformPlacement::PreMix).await;
}

#[tokio::test]
async fn buffered_post_mix_tail_is_drained_before_end() {
    assert_buffered_tail_is_drained(TransformPlacement::PostMix).await;
}

#[tokio::test]
async fn crossfade_runs_per_track_pre_mix_and_one_shared_post_mix_chain() {
    let samples = Arc::new(Mutex::new(Vec::new()));
    let pre_counts = Arc::new(Mutex::new(Vec::new()));
    let post_counts = Arc::new(Mutex::new(Vec::new()));
    let registry = StageRegistrySnapshot {
        decoders: vec![Arc::new(TestDecoderFactory::new())],
        // Register in reverse placement order to also cover deterministic planning.
        transforms: vec![
            Arc::new(CountingTransformFactory::new(
                "test.post-mix",
                TransformPlacement::PostMix,
                Arc::clone(&post_counts),
            )),
            Arc::new(CountingTransformFactory::new(
                "test.pre-mix",
                TransformPlacement::PreMix,
                Arc::clone(&pre_counts),
            )),
        ],
        sink: Arc::new(TestSinkFactory {
            id: StageId::new("test.sink").unwrap(),
            samples,
        }),
    };
    let mut config = PlaybackRuntimeConfig::new(registry);
    config.block_frames = 10;
    config.pcm_ring_blocks = 2;
    config.policies.transition = TransitionPolicy::Crossfade {
        duration_frames: 20,
        curve: CrossfadeCurve::EqualPower,
        fallback: CrossfadeFallback::Gapless,
    };
    let runtime = PlaybackRuntime::start(config).unwrap();
    let controller = runtime.controller();
    let mut events = controller.subscribe_events();
    controller
        .switch(item(1, 40, 100), SwitchOptions::default())
        .await
        .unwrap();
    controller.queue_next(item(2, 40, 50)).await.unwrap();
    wait_for_end(&mut events).await;

    let pre = pre_counts.lock().unwrap().clone();
    let post = post_counts.lock().unwrap().clone();
    assert_eq!(pre.len(), 2);
    assert!(pre.iter().all(|count| *count > 0));
    assert_eq!(post.len(), 2);
    assert!(post[0] > 0, "the shared current post-mix chain must run");
    assert_eq!(
        post[1], 0,
        "the next per-track path must not run post-mix DSP"
    );
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn crossfade_normalizes_next_sample_rate_and_channel_layout_before_mixing() {
    let formats = Arc::new(Mutex::new(Vec::new()));
    let creates = Arc::new(AtomicUsize::new(0));
    let sink = Arc::new(RecordingSinkFactory {
        id: StageId::new("test.recording-sink").unwrap(),
        formats: Arc::clone(&formats),
        creates: Arc::clone(&creates),
    });
    let registry = StageRegistrySnapshot {
        decoders: Vec::new(),
        transforms: Vec::new(),
        sink,
    };
    let mut config = PlaybackRuntimeConfig::new(registry);
    config.block_frames = 64;
    config.policies.transition = TransitionPolicy::Crossfade {
        duration_frames: 20,
        curve: CrossfadeCurve::Linear,
        fallback: CrossfadeFallback::Gapless,
    };
    let runtime = PlaybackRuntime::start(config).unwrap();
    let controller = runtime.controller();
    let mut events = controller.subscribe_events();
    let current_format = PcmFormat {
        sample_rate: 100,
        channel_layout: ChannelLayout::MONO,
    };
    let next_format = PcmFormat {
        sample_rate: 200,
        channel_layout: ChannelLayout::STEREO,
    };
    controller
        .switch(
            fixed_format_item(
                1,
                Arc::new(FixedFormatDecoderFactory::new(
                    "test.current-format",
                    current_format,
                    80,
                    1.0,
                )),
            ),
            SwitchOptions::default(),
        )
        .await
        .unwrap();
    controller
        .queue_next(fixed_format_item(
            2,
            Arc::new(FixedFormatDecoderFactory::new(
                "test.next-format",
                next_format,
                160,
                0.5,
            )),
        ))
        .await
        .unwrap();
    wait_for_end(&mut events).await;

    assert_eq!(creates.load(Ordering::SeqCst), 1, "output must be reused");
    let written_formats = formats.lock().unwrap().clone();
    assert!(!written_formats.is_empty());
    assert!(
        written_formats
            .iter()
            .all(|format| *format == current_format)
    );
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn gapless_reuses_sink_and_reports_consumed_boundary() {
    let samples = Arc::new(Mutex::new(Vec::new()));
    let runtime = runtime(TransitionPolicy::Gapless, Arc::clone(&samples));
    let controller = runtime.controller();
    let mut events = controller.subscribe_events();
    controller
        .switch(
            item(1, 40, 100),
            SwitchOptions {
                autoplay: false,
                ..SwitchOptions::default()
            },
        )
        .await
        .unwrap();
    controller.queue_next(item(2, 40, 50)).await.unwrap();
    controller.play().await.unwrap();
    wait_for_end(&mut events).await;
    let output = samples.lock().unwrap().clone();
    assert_eq!(output.len(), 80);
    assert_eq!(&output[..40], vec![1.0; 40]);
    assert_eq!(&output[40..], vec![0.5; 40]);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn fade_out_in_is_sequential_and_never_overlaps_track_pipelines() {
    let samples = Arc::new(Mutex::new(Vec::new()));
    let runtime = runtime(
        TransitionPolicy::FadeOutIn {
            fade_out_frames: 10,
            fade_in_frames: 10,
            curve: GainCurve::Linear,
        },
        Arc::clone(&samples),
    );
    let controller = runtime.controller();
    let mut events = controller.subscribe_events();
    controller
        .switch(
            item(1, 40, 100),
            SwitchOptions {
                autoplay: false,
                ..SwitchOptions::default()
            },
        )
        .await
        .unwrap();
    controller.queue_next(item(2, 40, 100)).await.unwrap();
    controller.play().await.unwrap();
    wait_for_end(&mut events).await;

    let output = samples.lock().unwrap().clone();
    assert_eq!(output.len(), 80, "sequential fade must not overlap PCM");
    assert!(output[30] > output[39], "current track must fade out");
    assert!(output[40] < output[49], "next track must fade in");
    assert_eq!(output[40], 0.0);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn eof_waits_in_buffering_for_slow_next_preparation_then_promotes() {
    let samples = Arc::new(Mutex::new(Vec::new()));
    let runtime = runtime(TransitionPolicy::Gapless, Arc::clone(&samples));
    let controller = runtime.controller();
    let mut events = controller.subscribe_events();
    controller
        .switch(item(1, 30, 100), SwitchOptions::default())
        .await
        .unwrap();
    let queued = {
        let controller = controller.clone();
        tokio::spawn(async move {
            controller
                .queue_next(delayed_item(2, 20, 50, Duration::from_millis(80)))
                .await
        })
    };

    let (saw_waiting, saw_next) = timeout(Duration::from_secs(3), async {
        let mut waiting = false;
        let mut next = false;
        loop {
            match events.recv().await.unwrap() {
                PlaybackEvent::Buffering {
                    item_id,
                    active: true,
                } if item_id == PlaybackItemId::new(1).unwrap() => waiting = true,
                PlaybackEvent::TrackChanged { item_id }
                    if item_id == PlaybackItemId::new(2).unwrap() =>
                {
                    next = true;
                },
                PlaybackEvent::PlaybackEnded { item_id }
                    if item_id == PlaybackItemId::new(2).unwrap() =>
                {
                    break (waiting, next);
                },
                _ => {},
            }
        }
    })
    .await
    .expect("slow next should eventually promote");
    queued.await.unwrap().unwrap();
    assert!(saw_waiting);
    assert!(saw_next);
    assert_eq!(samples.lock().unwrap().len(), 50);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn newer_switch_cancels_a_stale_slow_source_open() {
    let samples = Arc::new(Mutex::new(Vec::new()));
    let runtime = runtime(TransitionPolicy::Gapless, samples);
    let controller = runtime.controller();
    let first = {
        let controller = controller.clone();
        tokio::spawn(async move {
            controller
                .switch(
                    delayed_item(1, 20, 100, Duration::from_millis(500)),
                    SwitchOptions::default(),
                )
                .await
        })
    };
    tokio::time::sleep(Duration::from_millis(20)).await;
    timeout(
        Duration::from_millis(200),
        controller.switch(item(2, 20, 50), SwitchOptions::default()),
    )
    .await
    .expect("new switch must not wait for the stale source open")
    .unwrap();
    assert!(matches!(
        first.await.unwrap(),
        Err(PlaybackControlError::Closed)
    ));
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn crossfade_overlaps_two_track_pipelines() {
    let samples = Arc::new(Mutex::new(Vec::new()));
    let runtime = runtime(
        TransitionPolicy::Crossfade {
            duration_frames: 20,
            curve: CrossfadeCurve::Linear,
            fallback: CrossfadeFallback::Gapless,
        },
        Arc::clone(&samples),
    );
    let controller = runtime.controller();
    let mut events = controller.subscribe_events();
    controller
        .switch(
            item(1, 100, 100),
            SwitchOptions {
                autoplay: false,
                ..SwitchOptions::default()
            },
        )
        .await
        .unwrap();
    controller.queue_next(item(2, 100, 0)).await.unwrap();
    controller.play().await.unwrap();
    wait_for_end(&mut events).await;
    let output = samples.lock().unwrap().clone();
    assert_eq!(
        output.len(),
        180,
        "20 frames must overlap instead of concatenate"
    );
    assert!(output[80] > output[99]);
    assert!(output[99] < 0.1);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn next_failure_during_crossfade_is_typed_and_current_loses_no_frames() {
    let samples = Arc::new(Mutex::new(Vec::new()));
    let runtime = runtime(
        TransitionPolicy::Crossfade {
            duration_frames: 20,
            curve: CrossfadeCurve::Linear,
            fallback: CrossfadeFallback::Gapless,
        },
        Arc::clone(&samples),
    );
    let controller = runtime.controller();
    let mut events = controller.subscribe_events();
    controller
        .switch(item(1, 100, 100), SwitchOptions::default())
        .await
        .unwrap();
    controller
        .queue_next(fixed_format_item(
            2,
            Arc::new(FailingNextDecoderFactory::new()),
        ))
        .await
        .unwrap();

    let failure = timeout(Duration::from_secs(3), async {
        let mut failure = None;
        loop {
            match events.recv().await.unwrap() {
                PlaybackEvent::Failed(value) => failure = Some(value),
                PlaybackEvent::PlaybackEnded { .. } => break failure.unwrap(),
                _ => {},
            }
        }
    })
    .await
    .expect("current track should recover from next failure and end");
    assert_eq!(failure.item_id, PlaybackItemId::new(2));
    assert_eq!(failure.stage, stellatune_audio_core::FailureStage::Decoder);
    assert!(failure.generation > 0);
    assert_eq!(samples.lock().unwrap().len(), 100);
    runtime.shutdown().await.unwrap();
}
