use std::io::Read;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use stellatune_audio::planner::StageRegistrySnapshot;
use stellatune_audio::playback::control::SwitchOptions;
use stellatune_audio::playback::event::PlaybackState;
use stellatune_audio::playback::runtime::{PlaybackRuntime, PlaybackRuntimeConfig};
use stellatune_audio_core::{
    AudioBlock, ChannelLayout, DecodeError, DecodeStatus, DecodedStreamInfo, DecoderDescriptor,
    DecoderFactory, DecoderSeekStatus, FactoryError, MediaHints, OutputCompatibilityKey, PcmFormat,
    SeekResult, SinkClockSnapshot, SinkError, SinkFactory, SinkStage, SinkWriteResult,
    SinkWriteState, StageId,
};

use super::catalog::{MAX_PERSISTED_QUEUE_ITEMS, PlayerCatalog};
use super::error::PlayerServiceError;
use super::identity::{ProviderId, ProviderTrackIdentity, ProviderTrackKey, SourceInstanceId};
use super::resolver::{LocalTrackResolver, SourceResolver, SourceResolverFactory};
use super::service::PlayerService;
use super::source::{
    MediaHintsInput, ResolvedSourceSpec, SourceCatalogEntry, SourceResolutionInput,
    SourceResolverSpec,
};

fn resolver_spec(provider: &str) -> SourceResolverSpec {
    SourceResolverSpec::new(provider, "source", "{}").unwrap()
}

struct UnusedLocalResolver;

#[async_trait]
impl LocalTrackResolver for UnusedLocalResolver {
    async fn resolve_path(&self, _library_track_id: i64) -> Result<PathBuf, PlayerServiceError> {
        Err(PlayerServiceError::LocalTrackNotFound(0))
    }
}

struct FileLocalResolver {
    path: PathBuf,
    resolves: Arc<AtomicUsize>,
}

#[async_trait]
impl LocalTrackResolver for FileLocalResolver {
    async fn resolve_path(&self, _library_track_id: i64) -> Result<PathBuf, PlayerServiceError> {
        self.resolves.fetch_add(1, Ordering::SeqCst);
        Ok(self.path.clone())
    }
}

struct CountingResolverFactory {
    creates: Arc<AtomicUsize>,
    resolves: Arc<AtomicUsize>,
}

impl SourceResolverFactory for CountingResolverFactory {
    fn create(
        &self,
        _spec: &SourceResolverSpec,
    ) -> Result<Arc<dyn SourceResolver>, PlayerServiceError> {
        self.creates.fetch_add(1, Ordering::SeqCst);
        Ok(Arc::new(CountingResolver {
            resolves: Arc::clone(&self.resolves),
        }))
    }
}

struct CountingResolver {
    resolves: Arc<AtomicUsize>,
}

#[async_trait]
impl SourceResolver for CountingResolver {
    async fn resolve(
        &self,
        _source: &SourceCatalogEntry,
        _key: &ProviderTrackKey,
    ) -> Result<ResolvedSourceSpec, PlayerServiceError> {
        self.resolves.fetch_add(1, Ordering::SeqCst);
        SourceResolutionInput::Http {
            url: "https://temporary.invalid/audio?ephemeral=locator-secret".to_owned(),
            headers: std::collections::BTreeMap::from([(
                "Authorization".to_owned(),
                "Bearer ephemeral-header-secret".to_owned(),
            )]),
            media: MediaHintsInput {
                extension: Some("mp3".to_owned()),
                ..Default::default()
            },
            seekable: true,
            live: false,
        }
        .try_into()
    }
}

struct TestDecoderFactory {
    descriptor: DecoderDescriptor,
}

impl TestDecoderFactory {
    fn new() -> Self {
        Self {
            descriptor: DecoderDescriptor {
                id: StageId::new("test.restart-decoder").unwrap(),
                priority: 1,
                extensions: vec!["bin".to_owned()],
                mime_types: Vec::new(),
            },
        }
    }
}

impl DecoderFactory for TestDecoderFactory {
    fn descriptor(&self) -> &DecoderDescriptor {
        &self.descriptor
    }

    fn create(&self) -> Result<Box<dyn stellatune_audio_core::DecoderStage>, FactoryError> {
        Ok(Box::new(TestDecoder {
            total: 0,
            remaining: 0,
        }))
    }
}

struct TestDecoder {
    total: u64,
    remaining: u64,
}

impl stellatune_audio_core::DecoderStage for TestDecoder {
    fn open(
        &mut self,
        mut source: Box<dyn stellatune_audio_core::EncodedSource>,
        _hints: &MediaHints,
    ) -> Result<DecodedStreamInfo, DecodeError> {
        let mut header = [0_u8; 2];
        source.read_exact(&mut header).map_err(DecodeError::Io)?;
        self.total = u64::from(header[0]);
        self.remaining = self.total;
        Ok(DecodedStreamInfo {
            format: PcmFormat {
                sample_rate: 1_000,
                channel_layout: ChannelLayout::MONO,
            },
            duration_frames: Some(self.total),
            gapless_trim: None,
        })
    }

    fn decode(&mut self, output: &mut AudioBlock) -> Result<DecodeStatus, DecodeError> {
        if self.remaining == 0 {
            return Ok(DecodeStatus::EndOfStream);
        }
        let frames = self.remaining.min(10) as usize;
        output.samples.resize(frames, 1.0);
        self.remaining -= frames as u64;
        Ok(DecodeStatus::Produced { frames })
    }

    fn start_seek(&mut self, target_frame: u64) -> Result<DecoderSeekStatus, DecodeError> {
        let actual = target_frame.min(self.total);
        self.remaining = self.total.saturating_sub(actual);
        Ok(DecoderSeekStatus::Complete(SeekResult {
            actual_frame: actual,
        }))
    }

    fn continue_seek(&mut self) -> Result<DecoderSeekStatus, DecodeError> {
        Err(DecodeError::Unsupported)
    }

    fn reset(&mut self) {}
}

struct UnusedSinkFactory {
    id: StageId,
}

impl SinkFactory for UnusedSinkFactory {
    fn id(&self) -> &StageId {
        &self.id
    }

    fn compatibility_key(&self, format: PcmFormat) -> Result<OutputCompatibilityKey, FactoryError> {
        Ok(OutputCompatibilityKey {
            backend_id: "unused".to_owned(),
            device_id: None,
            sample_rate: format.sample_rate,
            channel_layout: format.channel_layout,
            route_revision: 0,
        })
    }

    fn create(&self) -> Result<Box<dyn SinkStage>, FactoryError> {
        Ok(Box::new(TestSink { consumed: 0 }))
    }
}

struct TestSink {
    consumed: u64,
}

impl SinkStage for TestSink {
    fn open(&mut self, _format: PcmFormat) -> Result<(), SinkError> {
        Ok(())
    }

    fn write(&mut self, block: &AudioBlock) -> Result<SinkWriteResult, SinkError> {
        self.consumed = self.consumed.saturating_add(block.frames() as u64);
        Ok(SinkWriteResult {
            consumed_frames: block.frames(),
            state: SinkWriteState::Ready,
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
        self.consumed = 0;
        Ok(())
    }
    fn clock_snapshot(&self) -> SinkClockSnapshot {
        SinkClockSnapshot {
            consumed_frames: self.consumed,
            buffered_frames: 0,
            epoch: 0,
        }
    }
    fn close(&mut self) {}
}

fn test_runtime() -> PlaybackRuntime {
    PlaybackRuntime::start(PlaybackRuntimeConfig::new(StageRegistrySnapshot {
        decoders: vec![Arc::new(TestDecoderFactory::new())],
        transforms: Vec::new(),
        sink: Arc::new(UnusedSinkFactory {
            id: StageId::new("test.restart-sink").unwrap(),
        }),
    }))
    .unwrap()
}

#[tokio::test]
async fn catalog_bootstrap_allocates_stable_typed_ids() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("player.sqlite");
    let catalog = PlayerCatalog::open(&path).await.unwrap();
    let local_source = catalog.ensure_local_source().await.unwrap();
    let local_track = catalog.ensure_local_track(local_source, 42).await.unwrap();
    let repeated = catalog.ensure_local_track(local_source, 42).await.unwrap();
    assert_eq!(local_track, repeated);

    let first_item = catalog.enqueue(local_track).await.unwrap();
    let second_item = catalog.enqueue(local_track).await.unwrap();
    assert_ne!(first_item, second_item);
    drop(catalog);

    let reopened = PlayerCatalog::open(&path).await.unwrap();
    assert_eq!(reopened.ensure_local_source().await.unwrap(), local_source);
    assert_eq!(
        reopened.ensure_local_track(local_source, 42).await.unwrap(),
        local_track
    );
    let third_item = reopened.enqueue(local_track).await.unwrap();
    assert!(third_item.get() > second_item.get());
}

#[tokio::test]
async fn persisted_queue_is_bounded_and_tombstones_never_reuse_ids() {
    let directory = tempfile::tempdir().unwrap();
    let catalog = PlayerCatalog::open(directory.path().join("player.sqlite"))
        .await
        .unwrap();
    let source = catalog.ensure_local_source().await.unwrap();
    let track = catalog.ensure_local_track(source, 77).await.unwrap();
    let mut latest = None;
    for _ in 0..(MAX_PERSISTED_QUEUE_ITEMS + 8) {
        latest = Some(catalog.enqueue(track).await.unwrap());
    }
    let state = catalog.load_state().await.unwrap();
    assert_eq!(state.queue.len() as i64, MAX_PERSISTED_QUEUE_ITEMS);
    assert_eq!(state.queue.last().map(|entry| entry.item_id), latest);

    catalog.tombstone_track(track).await.unwrap();
    assert_eq!(
        catalog.ensure_local_track(source, 77).await.unwrap(),
        track,
        "a tombstoned identity must not be reassigned"
    );
    assert!(matches!(
        catalog.enqueue(track).await,
        Err(PlayerServiceError::TrackUnavailable(id)) if id == track
    ));
    catalog.tombstone_source(source).await.unwrap();
    assert!(catalog.source(source).await.unwrap().tombstoned);
    assert!(catalog.track(track).await.unwrap().tombstoned);
}

#[tokio::test]
async fn provider_keys_are_scoped_by_source_instance() {
    let directory = tempfile::tempdir().unwrap();
    let catalog = PlayerCatalog::open(directory.path().join("player.sqlite"))
        .await
        .unwrap();
    let source_a = catalog
        .ensure_plugin_source(
            ProviderId::new("provider-a").unwrap(),
            resolver_spec("provider-a"),
        )
        .await
        .unwrap();
    let source_b = catalog
        .ensure_plugin_source(
            ProviderId::new("provider-b").unwrap(),
            resolver_spec("provider-b"),
        )
        .await
        .unwrap();
    let key = ProviderTrackKey::Text("same-key".to_owned());
    let track_a = catalog
        .ensure_track(ProviderTrackIdentity {
            source_instance_id: source_a,
            provider_key: key.clone(),
        })
        .await
        .unwrap();
    let track_a_again = catalog
        .ensure_track(ProviderTrackIdentity {
            source_instance_id: source_a,
            provider_key: key.clone(),
        })
        .await
        .unwrap();
    let track_b = catalog
        .ensure_track(ProviderTrackIdentity {
            source_instance_id: source_b,
            provider_key: key,
        })
        .await
        .unwrap();
    assert_eq!(track_a, track_a_again);
    assert_ne!(track_a, track_b);
}

#[tokio::test]
async fn mismatched_or_partial_player_schema_is_rejected_without_mutation() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("player.sqlite");
    let pool = sqlx::SqlitePool::connect(&format!("sqlite:{}?mode=rwc", path.display()))
        .await
        .unwrap();
    sqlx::query("CREATE TABLE playback_queue(item_id INTEGER PRIMARY KEY)")
        .execute(&pool)
        .await
        .unwrap();
    drop(pool);

    assert!(matches!(
        PlayerCatalog::open(&path).await,
        Err(PlayerServiceError::IncompatiblePlaybackSchema)
    ));
    let pool = sqlx::SqlitePool::connect(&format!("sqlite:{}", path.display()))
        .await
        .unwrap();
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='player_schema_meta'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn restart_recreates_provider_resolver_without_persisting_temporary_locator() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("player.sqlite");
    let catalog = PlayerCatalog::open(&path).await.unwrap();
    let source = catalog
        .ensure_plugin_source(
            ProviderId::new("provider-restart").unwrap(),
            resolver_spec("plugin-restart"),
        )
        .await
        .unwrap();
    let track = catalog
        .ensure_track(ProviderTrackIdentity {
            source_instance_id: source,
            provider_key: ProviderTrackKey::Text("stable-provider-key".to_owned()),
        })
        .await
        .unwrap();
    let item = catalog.enqueue(track).await.unwrap();
    drop(catalog);

    let reopened = PlayerCatalog::open(&path).await.unwrap();
    let creates = Arc::new(AtomicUsize::new(0));
    let resolves = Arc::new(AtomicUsize::new(0));
    let runtime = PlaybackRuntime::start(PlaybackRuntimeConfig::new(StageRegistrySnapshot {
        decoders: Vec::new(),
        transforms: Vec::new(),
        sink: Arc::new(UnusedSinkFactory {
            id: StageId::new("test.unused-sink").unwrap(),
        }),
    }))
    .unwrap();
    let service = PlayerService::new(
        reopened.clone(),
        runtime.controller(),
        Arc::new(UnusedLocalResolver),
        Arc::new(CountingResolverFactory {
            creates: Arc::clone(&creates),
            resolves: Arc::clone(&resolves),
        }),
    );
    let materialized = service.materialize_item(item, track).await.unwrap();
    assert_eq!(materialized.id, item);
    assert_eq!(creates.load(Ordering::SeqCst), 1);
    assert_eq!(resolves.load(Ordering::SeqCst), 1);

    let source_entry = reopened.source(source).await.unwrap();
    let persisted = source_entry.resolver.unwrap();
    assert!(!persisted.config_json.contains("locator-secret"));
    assert!(!persisted.config_json.contains("header-secret"));
    let schema = sqlx::query_scalar::<_, String>(
        "SELECT sql FROM sqlite_master WHERE type='table' AND name='source_catalog'",
    )
    .fetch_one(&reopened.pool)
    .await
    .unwrap();
    assert!(!schema.contains("url"));
    assert!(!schema.contains("headers"));
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn restart_restores_local_item_and_sink_consumed_position_as_paused() {
    let directory = tempfile::tempdir().unwrap();
    let database_path = directory.path().join("player.sqlite");
    let media_path = directory.path().join("fixture.bin");
    std::fs::write(&media_path, [100_u8, 100_u8]).unwrap();
    let local_resolves = Arc::new(AtomicUsize::new(0));

    let catalog = PlayerCatalog::open(&database_path).await.unwrap();
    let runtime = test_runtime();
    let service = PlayerService::new(
        catalog.clone(),
        runtime.controller(),
        Arc::new(FileLocalResolver {
            path: media_path.clone(),
            resolves: Arc::clone(&local_resolves),
        }),
        Arc::new(CountingResolverFactory {
            creates: Arc::new(AtomicUsize::new(0)),
            resolves: Arc::new(AtomicUsize::new(0)),
        }),
    );
    let track = service.ensure_local_track(7).await.unwrap();
    let item = service
        .switch_track(
            track,
            SwitchOptions {
                autoplay: false,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    service
        .controller
        .seek(stellatune_audio_core::MediaTime::from_millis(40))
        .await
        .unwrap();
    let snapshot = service.controller.snapshot().await.unwrap();
    service
        .catalog
        .save_runtime_state(snapshot, false)
        .await
        .unwrap();
    runtime.shutdown().await.unwrap();
    drop(service);
    drop(catalog);

    let reopened = PlayerCatalog::open(&database_path).await.unwrap();
    let restarted_runtime = test_runtime();
    let restarted = PlayerService::new(
        reopened,
        restarted_runtime.controller(),
        Arc::new(FileLocalResolver {
            path: media_path,
            resolves: Arc::clone(&local_resolves),
        }),
        Arc::new(CountingResolverFactory {
            creates: Arc::new(AtomicUsize::new(0)),
            resolves: Arc::new(AtomicUsize::new(0)),
        }),
    );
    let restored = restarted.restore().await.unwrap();
    assert_eq!(restored.current_item_id, Some(item));
    assert_eq!(restored.position_ms, 40);
    assert_eq!(restored.queue[0].item_id, item);
    assert_eq!(restored.queue[0].track_id, track);
    let snapshot = restarted.controller.snapshot().await.unwrap();
    assert_eq!(snapshot.current_item_id, Some(item));
    assert_eq!(snapshot.consumed_position.as_millis(), 40);
    assert_eq!(snapshot.state, PlaybackState::Ready);
    assert_eq!(local_resolves.load(Ordering::SeqCst), 2);
    restarted_runtime.shutdown().await.unwrap();
}

#[test]
fn identity_types_reject_zero_and_sqlite_overflow() {
    assert!(SourceInstanceId::new(0).is_err());
    assert!(SourceInstanceId::new(i64::MAX as u64 + 1).is_err());
}
