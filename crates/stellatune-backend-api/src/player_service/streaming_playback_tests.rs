use super::*;
use std::time::{Duration, Instant};
use stellatune_audio_builtin_adapters::factories::{FileSourceFactory, SymphoniaDecoderFactory};
use stellatune_audio_core::playback::{PlaybackItem, PlaybackItemId};
use stellatune_audio_core::source::SourceFactory;

struct SharedFormatSink(UnusedSinkFactory);
impl SinkFactory for SharedFormatSink {
    fn id(&self) -> &StageId {
        self.0.id()
    }
    fn preferred_format(&self, _input: PcmFormat) -> Result<PcmFormat, FactoryError> {
        Ok(PcmFormat {
            sample_rate: 48_000,
            channel_layout: ChannelLayout::STEREO,
        })
    }
    fn compatibility_key(&self, format: PcmFormat) -> Result<OutputCompatibilityKey, FactoryError> {
        self.0.compatibility_key(format)
    }
    fn create(&self) -> Result<Box<dyn SinkStage>, FactoryError> {
        self.0.create()
    }
}

// The sink accepts immediately: this measures whether the complete actor,
// asynchronous decoder and resampler can supply PCM faster than real time.
// Counting the whole file offline alone does not detect timer-limited playback.
async fn assert_realtime_supply(source: Arc<dyn SourceFactory>) {
    let runtime = PlaybackRuntime::start(PlaybackRuntimeConfig::new(StageRegistrySnapshot {
        decoders: vec![Arc::new(SymphoniaDecoderFactory::new())],
        transforms: vec![],
        sink: Arc::new(SharedFormatSink(UnusedSinkFactory {
            id: StageId::new("test.throughput").unwrap(),
        })),
    }))
    .unwrap();
    let controller = runtime.controller();
    controller
        .switch_to(
            PlaybackItem {
                id: PlaybackItemId::new(1).unwrap(),
                source,
                required_decoder: None,
            },
            SwitchOptions::default(),
        )
        .await
        .unwrap();
    let started = Instant::now();
    tokio::time::sleep(Duration::from_secs(2)).await;
    let snapshot = controller.snapshot().await.unwrap();
    let elapsed = started.elapsed();
    runtime.shutdown().await.unwrap();
    eprintln!(
        "PCM supplied {} ms in {} ms (48 kHz stereo output)",
        snapshot.consumed_position.as_millis(),
        elapsed.as_millis()
    );
    assert!(
        snapshot.consumed_position.as_millis() > elapsed.as_millis() as u64 * 2,
        "decoder/pump must retain supply headroom: {snapshot:?}, elapsed {elapsed:?}"
    );
}

#[tokio::test]
async fn high_rate_pcm_supply_is_not_limited_to_one_block_per_tick() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("192khz.wav");
    let sample_rate = 192_000_u32;
    let length = sample_rate * 20 * 4;
    let mut wave = Vec::new();
    wave.extend_from_slice(b"RIFF");
    wave.extend_from_slice(&(36 + length).to_le_bytes());
    wave.extend_from_slice(b"WAVEfmt ");
    wave.extend_from_slice(&16_u32.to_le_bytes());
    wave.extend_from_slice(&1_u16.to_le_bytes());
    wave.extend_from_slice(&2_u16.to_le_bytes());
    wave.extend_from_slice(&sample_rate.to_le_bytes());
    wave.extend_from_slice(&(sample_rate * 4).to_le_bytes());
    wave.extend_from_slice(&4_u16.to_le_bytes());
    wave.extend_from_slice(&16_u16.to_le_bytes());
    wave.extend_from_slice(b"data");
    wave.extend_from_slice(&length.to_le_bytes());
    wave.resize(44 + length as usize, 0);
    std::fs::write(&path, wave).unwrap();
    assert_realtime_supply(Arc::new(
        FileSourceFactory::new(path, Default::default()).unwrap(),
    ))
    .await;
}

#[tokio::test]
#[ignore = "set STELLATUNE_NCM_TEST_FILE to a local NCM file"]
async fn real_ncm_stream_supplies_shared_output_faster_than_realtime() {
    use stellatune_plugins::typescript::TypeScriptRuntime;
    let root = tempfile::tempdir().unwrap();
    let package = root.path().join("plugin");
    super::local_plugin_tests::prepare_package(&package);
    let plugins = Arc::new(TypeScriptRuntime::new(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tools/typescript-plugin-runtime/runner.mjs"),
    ));
    plugins.configure_host("http://127.0.0.1:1".into(), root.path().join("data"));
    let manifest = stellatune_plugins::typescript::manifest::read_typescript_manifest(
        &package.join("manifest.json"),
    )
    .unwrap();
    plugins.register(manifest, package).await.unwrap();
    let path = PathBuf::from(std::env::var_os("STELLATUNE_NCM_TEST_FILE").unwrap());
    let resolved = crate::runtime::local_source::resolve_local_file(&plugins, &path)
        .await
        .unwrap();
    let source = super::super::resolver::materialize_source(resolved.source).unwrap();
    assert_realtime_supply(source).await;
    plugins
        .unregister("dev.stellatune.source.ncm")
        .await
        .unwrap();
}

struct HeldOutput {
    factory: UnusedSinkFactory,
    frames: Arc<AtomicUsize>,
}
impl SinkFactory for HeldOutput {
    fn id(&self) -> &StageId {
        self.factory.id()
    }
    fn preferred_format(&self, _: PcmFormat) -> Result<PcmFormat, FactoryError> {
        Ok(PcmFormat {
            sample_rate: 48000,
            channel_layout: ChannelLayout::STEREO,
        })
    }
    fn compatibility_key(&self, format: PcmFormat) -> Result<OutputCompatibilityKey, FactoryError> {
        self.factory.compatibility_key(format)
    }
    fn create(&self) -> Result<Box<dyn SinkStage>, FactoryError> {
        Ok(Box::new(HeldSink(self.frames.clone())))
    }
}
struct HeldSink(Arc<AtomicUsize>);
impl SinkStage for HeldSink {
    fn open(&mut self, _: PcmFormat) -> Result<(), SinkError> {
        Ok(())
    }
    fn write(
        &mut self,
        block: &AudioBlock,
    ) -> Result<stellatune_audio_core::sink::SinkWriteResult, SinkError> {
        self.0.fetch_add(block.frames(), Ordering::Release);
        Ok(stellatune_audio_core::sink::SinkWriteResult {
            consumed_frames: block.frames(),
            state: stellatune_audio_core::sink::SinkWriteState::Ready,
        })
    }
    fn pause(&mut self) -> Result<(), SinkError> {
        Ok(())
    }
    fn resume(&mut self) -> Result<(), SinkError> {
        Ok(())
    }
    fn drain(&mut self) -> Result<(), SinkError> {
        Ok(())
    }
    fn discard(&mut self) -> Result<(), SinkError> {
        self.0.store(0, Ordering::Release);
        Ok(())
    }
    fn clock_snapshot(&self) -> SinkClockSnapshot {
        SinkClockSnapshot {
            buffered_frames: self.0.load(Ordering::Acquire) as u64,
            ..Default::default()
        }
    }
    fn close(&mut self) {}
}

#[tokio::test]
async fn decoder_and_resampler_fill_shared_output_duration_for_all_latency_presets() {
    use stellatune_audio_core::buffering::{LatencyProfile, frames_for_ms};
    let root = tempfile::tempdir().unwrap();
    for rate in [8000_u32, 44100, 48000, 96000, 192000] {
        let path = root.path().join(format!("{rate}.wav"));
        let length = rate * 2 * 4;
        let mut wave = Vec::new();
        wave.extend_from_slice(b"RIFF");
        wave.extend_from_slice(&(36 + length).to_le_bytes());
        wave.extend_from_slice(b"WAVEfmt ");
        wave.extend_from_slice(&16_u32.to_le_bytes());
        wave.extend_from_slice(&1_u16.to_le_bytes());
        wave.extend_from_slice(&2_u16.to_le_bytes());
        wave.extend_from_slice(&rate.to_le_bytes());
        wave.extend_from_slice(&(rate * 4).to_le_bytes());
        wave.extend_from_slice(&4_u16.to_le_bytes());
        wave.extend_from_slice(&16_u16.to_le_bytes());
        wave.extend_from_slice(b"data");
        wave.extend_from_slice(&length.to_le_bytes());
        wave.resize(44 + length as usize, 0);
        std::fs::write(&path, wave).unwrap();
        for profile in [
            LatencyProfile::Low,
            LatencyProfile::Medium,
            LatencyProfile::High,
        ] {
            let frames = Arc::new(AtomicUsize::new(0));
            let mut config = PlaybackRuntimeConfig::new(StageRegistrySnapshot {
                decoders: vec![Arc::new(SymphoniaDecoderFactory::new())],
                transforms: vec![],
                sink: Arc::new(HeldOutput {
                    factory: UnusedSinkFactory {
                        id: StageId::new("test.held").unwrap(),
                    },
                    frames: frames.clone(),
                }),
            });
            config.buffering = profile.buffering();
            let output = PcmFormat {
                sample_rate: 48000,
                channel_layout: ChannelLayout::STEREO,
            };
            let target = frames_for_ms(output, config.buffering.output_ms);
            let block = frames_for_ms(output, config.buffering.block_ms);
            let runtime = PlaybackRuntime::start(config).unwrap();
            let controller = runtime.controller();
            controller
                .switch_to(
                    PlaybackItem {
                        id: PlaybackItemId::new(1).unwrap(),
                        source: Arc::new(
                            FileSourceFactory::new(path.clone(), Default::default()).unwrap(),
                        ),
                        required_decoder: None,
                    },
                    SwitchOptions::default(),
                )
                .await
                .unwrap();
            tokio::time::timeout(Duration::from_secs(3), async {
                while frames.load(Ordering::Acquire) < target {
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            })
            .await
            .unwrap();
            tokio::time::sleep(Duration::from_millis(120)).await;
            let supplied = frames.load(Ordering::Acquire);
            assert!(
                supplied <= target + block + 6,
                "{rate} Hz -> 48 kHz {profile:?}: {supplied} exceeds {target} + {block}"
            );
            runtime.shutdown().await.unwrap();
        }
    }
}
