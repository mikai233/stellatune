use super::support::{FixedFormatDecoderFactory, TestSinkFactory, fixed_format_item, wait_for_end};
use crate::{
    planner::{StageRegistrySnapshot, TransitionPolicy},
    playback::{
        control::SwitchOptions,
        runtime::{PlaybackRuntime, PlaybackRuntimeConfig},
    },
};
use std::sync::{Arc, Mutex};
use stellatune_audio_core::{
    format::{ChannelLayout, PcmFormat},
    stage::StageId,
};

#[tokio::test]
async fn startup_fade_uses_pcm_time_at_each_rate_and_does_not_fade_gapless_successor() {
    for rate in [8000, 44100, 48000, 96000, 192000] {
        let samples = Arc::new(Mutex::new(Vec::new()));
        let format = PcmFormat {
            sample_rate: rate,
            channel_layout: ChannelLayout::STEREO,
        };
        let frames = u64::from(rate) / 50;
        let decoder = Arc::new(FixedFormatDecoderFactory::new(
            "test.startup",
            format,
            frames,
            0.5,
        ));
        let mut config = PlaybackRuntimeConfig::new(StageRegistrySnapshot {
            decoders: vec![decoder.clone()],
            transforms: vec![],
            sink: Arc::new(TestSinkFactory {
                id: StageId::new("test.startup-sink").unwrap(),
                samples: samples.clone(),
            }),
        });
        config.policies.transition = TransitionPolicy::Gapless;
        config.policies.seek_fade_frames = 0; // Startup protection is independent of seek preferences.
        let runtime = PlaybackRuntime::start(config).unwrap();
        let controller = runtime.controller();
        let mut events = controller.subscribe_events();
        controller
            .switch_to(
                fixed_format_item(1, decoder.clone()),
                SwitchOptions {
                    autoplay: false,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        controller
            .set_next(Some(fixed_format_item(2, decoder)))
            .await
            .unwrap();
        // Wall-clock waiting before play must not consume the PCM envelope.
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        controller.play().await.unwrap();
        wait_for_end(&mut events).await;
        runtime.shutdown().await.unwrap();
        let output = samples.lock().unwrap();
        assert_eq!(
            output.len(),
            frames as usize * 4,
            "fade must not drop or add samples"
        );
        let fade_frames = (u64::from(rate) * 5 / 1000) as usize;
        for (index, frame) in output.as_chunks::<2>().0.iter().enumerate() {
            let expected = if index < fade_frames {
                0.5 * index as f32 / fade_frames as f32
            } else {
                0.5
            };
            for sample in frame {
                assert!(
                    (sample - expected).abs() < 1e-6,
                    "{rate} Hz frame {index}: {sample} != {expected}"
                );
            }
        }
    }
}

#[tokio::test]
async fn startup_fade_after_resampling_is_five_milliseconds_at_the_mix_rate() {
    use super::support::FormatAdaptingSinkFactory;
    let samples = Arc::new(Mutex::new(Vec::new()));
    let formats = Arc::new(Mutex::new(Vec::new()));
    let decoder = Arc::new(FixedFormatDecoderFactory::new(
        "test.startup-96k",
        PcmFormat {
            sample_rate: 96000,
            channel_layout: ChannelLayout::STEREO,
        },
        9600,
        0.5,
    ));
    let mut config = PlaybackRuntimeConfig::new(StageRegistrySnapshot {
        decoders: vec![decoder.clone()],
        transforms: vec![],
        sink: Arc::new(FormatAdaptingSinkFactory {
            id: StageId::new("test.startup-48k").unwrap(),
            target: PcmFormat {
                sample_rate: 48000,
                channel_layout: ChannelLayout::STEREO,
            },
            formats: formats.clone(),
            samples: samples.clone(),
        }),
    });
    config.policies.seek_fade_frames = 0;
    let runtime = PlaybackRuntime::start(config).unwrap();
    let controller = runtime.controller();
    let mut events = controller.subscribe_events();
    controller
        .switch_to(fixed_format_item(1, decoder), SwitchOptions::default())
        .await
        .unwrap();
    wait_for_end(&mut events).await;
    runtime.shutdown().await.unwrap();
    let output = samples.lock().unwrap();
    assert_eq!(output.len(), 4800 * 2);
    assert_eq!(&output[..2], &[0.0, 0.0]);
    // Away from the resampler's initial filter transient, gain follows output time.
    for frame in [120, 180, 239, 240, 300, 1000] {
        let expected = 0.5 * (frame as f32 / 240.0).min(1.0);
        for sample in &output[frame * 2..frame * 2 + 2] {
            assert!(
                (sample - expected).abs() < 1e-4,
                "frame {frame}: {sample} != {expected}"
            );
        }
    }
}
