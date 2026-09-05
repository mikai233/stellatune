use super::*;
use stellatune_audio::planner::{PipelinePlanner, PlaybackRequest};
use stellatune_audio_builtin_adapters::factories::SymphoniaDecoderFactory;
use stellatune_audio_core::source::{SourceCancellation, SourceOpenPurpose, SourceOpenRequest};
use stellatune_plugins::typescript::{
    TypeScriptRuntime,
    package::{install_typescript_artifact, uninstall_typescript_plugin},
};

const PLUGIN: &str = "dev.stellatune.source.ncm";

async fn exercise_local_plugin(path: PathBuf, expected_frames: Option<u64>) {
    let directory = tempfile::tempdir().unwrap();
    let payload = directory.path().join("payload");
    prepare_package(&payload);
    let plugins_dir = directory.path().join("installed");
    let installed = install_typescript_artifact(&plugins_dir, &payload).unwrap();
    let plugins = Arc::new(TypeScriptRuntime::new(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tools/typescript-plugin-runtime/runner.mjs"),
    ));
    plugins.configure_host(
        "http://127.0.0.1:1".to_owned(),
        directory.path().join("plugin-data"),
    );
    let catalog = PlayerCatalog::open(directory.path().join("player.sqlite"))
        .await
        .unwrap();
    let registry = StageRegistrySnapshot {
        decoders: vec![Arc::new(SymphoniaDecoderFactory::new())],
        transforms: vec![],
        sink: Arc::new(UnusedSinkFactory {
            id: StageId::new("test.local-source-sink").unwrap(),
        }),
    };
    let playback = PlaybackRuntime::start(PlaybackRuntimeConfig::new(registry.clone())).unwrap();
    let service = Arc::new(PlayerService::new(
        catalog,
        playback.controller(),
        Arc::new(FileLocalResolver {
            path: path.clone(),
            resolves: Arc::new(AtomicUsize::new(0)),
        }),
        Arc::new(crate::runtime::TypeScriptSourceResolverFactory::new(
            plugins.clone(),
        )),
    ));
    let track = service.ensure_local_tracks(&[42]).await.unwrap()[0];
    let item = service.append_queue(vec![track]).await.unwrap().items[0].item_id;
    assert!(
        service
            .materialize_item(item, track)
            .await
            .unwrap_err()
            .to_string()
            .contains("no enabled local-source plugin")
    );

    plugins
        .register(installed.manifest.clone(), &installed.root_dir)
        .await
        .unwrap();
    assert_eq!(plugins.local_file_extensions(), ["ncm"]);
    let resolved = service.materialize_item(item, track).await.unwrap();
    assert_eq!(resolved.id, item);
    assert_ne!(
        resolved.source.descriptor().media.extension.as_deref(),
        Some("ncm")
    );
    let plan = PipelinePlanner
        .plan(
            PlaybackRequest {
                item: resolved,
                policies: Default::default(),
            },
            &registry,
        )
        .unwrap();
    let hints = plan.item.source.descriptor().media;
    let source = plan
        .item
        .source
        .open(SourceOpenRequest {
            purpose: SourceOpenPurpose::Initial,
            deadline: None,
            cancellation: SourceCancellation::default(),
        })
        .await
        .unwrap();
    let mut decoder = plan.decoder_candidates[0].create().unwrap();
    let (mut decoder, info) = tokio::task::spawn_blocking(move || {
        let info = decoder.open(source, &hints).unwrap();
        (decoder, info)
    })
    .await
    .unwrap();
    let mut block = AudioBlock::new(info.format);
    block
        .samples
        .reserve(16384 * usize::from(info.format.channel_layout.channel_count()));
    let mut total_frames = 0;
    let mut sample_prefix = Vec::new();
    loop {
        match decoder.decode(&mut block).unwrap() {
            DecodeStatus::Produced { frames } => {
                total_frames += frames as u64;
                if sample_prefix.len() < 16000 {
                    sample_prefix.extend_from_slice(&block.samples);
                }
            },
            DecodeStatus::EndOfStream => break,
            DecodeStatus::Pending => tokio::time::sleep(std::time::Duration::from_millis(2)).await,
        }
    }
    assert!(total_frames > 0);
    if let Some(expected) = expected_frames {
        assert_eq!(total_frames, expected);
    }
    let target = if expected_frames.is_some() {
        4000
    } else {
        (u64::from(info.format.sample_rate) * 60).min(total_frames / 2)
    };
    let mut status = decoder.start_seek(target).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while matches!(status, DecoderSeekStatus::Pending) {
        assert!(std::time::Instant::now() < deadline);
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        status = decoder.continue_seek().unwrap();
    }
    loop {
        match decoder.decode(&mut block).unwrap() {
            DecodeStatus::Produced { .. } => break,
            DecodeStatus::Pending => {
                assert!(std::time::Instant::now() < deadline);
                tokio::time::sleep(std::time::Duration::from_millis(2)).await;
            },
            DecodeStatus::EndOfStream => panic!("seek must produce audio"),
        }
    }
    if expected_frames.is_some() {
        assert_eq!(
            block.samples,
            sample_prefix[4000..4000 + block.samples.len()]
        );
    }
    eprintln!(
        "NCM plugin: {} Hz, {} channels, {} frames; seek {} frames decoded successfully",
        info.format.sample_rate,
        info.format.channel_layout.channel_count(),
        total_frames,
        target
    );
    drop(decoder);
    drop(plan);

    assert_eq!(
        service.local_path_for_item(item).await.unwrap(),
        Some(path.clone())
    );
    {
        let resolved = crate::runtime::local_source::resolve_local_file(&plugins, &path)
            .await
            .unwrap();
        let mut opened = crate::runtime::local_decoder::open_resolved_decoder(resolved)
            .await
            .unwrap();
        assert_eq!(opened.plugin_id.as_deref(), Some(PLUGIN));
        assert_eq!(opened.decoder.spec(), info.format);
        let frames = tokio::task::spawn_blocking(move || {
            let mut frames = 0;
            while let Some(samples) = opened.decoder.next_block(500).unwrap() {
                frames += samples.len() as u64
                    / u64::from(opened.decoder.spec().channel_layout.channel_count());
            }
            frames
        })
        .await
        .unwrap();
        assert_eq!(
            frames, total_frames,
            "blocking probe/transcode path shares the HTTP source"
        );
    }
    if expected_frames.is_some() {
        use stellatune_audio::playback::event::PlaybackEvent;
        let mut events = playback.controller().subscribe_events();
        service
            .select_item(item, SwitchOptions::default())
            .await
            .unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                match events.recv().await.unwrap() {
                    PlaybackEvent::PlaybackEnded { item_id } => {
                        assert_eq!(item_id, item);
                        break;
                    },
                    PlaybackEvent::Failed(error) => panic!("{error}"),
                    _ => {},
                }
            }
        })
        .await
        .unwrap();
    }
    plugins.unregister(PLUGIN).await.unwrap();
    assert!(plugins.local_file_extensions().is_empty());
    assert!(
        service.materialize_item(item, track).await.is_err(),
        "HTTP sources must not bypass disabled plugin lookup"
    );
    assert_eq!(service.ensure_local_tracks(&[42]).await.unwrap()[0], track);
    plugins
        .register(installed.manifest, &installed.root_dir)
        .await
        .unwrap();
    assert!(service.materialize_item(item, track).await.is_ok());
    plugins.unregister(PLUGIN).await.unwrap();
    uninstall_typescript_plugin(&plugins_dir, PLUGIN).unwrap();
    assert!(!installed.root_dir.exists());
    assert!(
        !directory
            .path()
            .join("plugin-data")
            .join(PLUGIN)
            .join("cache")
            .exists()
    );
    playback.shutdown().await.unwrap();
}

#[tokio::test]
async fn optional_local_plugin_preserves_identity_and_decodes_seeks_and_disables() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("合成音频.NCM");
    std::fs::write(
        &path,
        include_bytes!("../../../plugins-native/stellatune-plugin-ncm/tests/fixtures/tone.ncm"),
    )
    .unwrap();
    exercise_local_plugin(path, Some(16000)).await;
}

#[tokio::test]
async fn optional_local_plugin_supports_mp3_payloads() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("tone.ncm");
    std::fs::write(
        &path,
        include_bytes!("../../../plugins-native/stellatune-plugin-ncm/tests/fixtures/tone-mp3.ncm"),
    )
    .unwrap();
    exercise_local_plugin(path, None).await;
}

#[tokio::test]
#[ignore = "requires STELLATUNE_NCM_TEST_FILE pointing to a local user-provided file"]
async fn local_user_ncm_file_decodes_and_seeks_through_plugin() {
    exercise_local_plugin(
        PathBuf::from(
            std::env::var_os("STELLATUNE_NCM_TEST_FILE").expect("set STELLATUNE_NCM_TEST_FILE"),
        ),
        None,
    )
    .await;
}

#[tokio::test]
async fn library_scans_local_plugin_metadata_only_while_enabled() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("music");
    std::fs::create_dir(&root).unwrap();
    let path = root.join("tone.ncm");
    std::fs::write(
        &path,
        include_bytes!("../../../plugins-native/stellatune-plugin-ncm/tests/fixtures/tone.ncm"),
    )
    .unwrap();
    let package = directory.path().join("package");
    prepare_package(&package);
    let manifest = stellatune_plugins::typescript::manifest::read_typescript_manifest(
        &package.join("manifest.json"),
    )
    .unwrap();
    let plugins = Arc::new(TypeScriptRuntime::new(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tools/typescript-plugin-runtime/runner.mjs"),
    ));
    plugins.configure_host(
        "http://127.0.0.1:1".to_owned(),
        directory.path().join("data"),
    );
    let provider = Arc::new(crate::runtime::local_source::PluginMetadataProvider::new(
        plugins.clone(),
    ));
    let library = stellatune_library::start_library_with_metadata_provider(
        directory
            .path()
            .join("library.sqlite")
            .to_string_lossy()
            .into_owned(),
        Some(provider),
    )
    .await
    .unwrap();
    let folder = root.to_string_lossy().into_owned();
    library.add_root(folder.clone()).await.unwrap();
    library.scan_all().await.unwrap();
    assert!(
        library
            .list_tracks(folder.clone(), true, String::new(), 10, 0)
            .await
            .unwrap()
            .is_empty()
    );
    plugins.register(manifest, package).await.unwrap();
    library.scan_all().await.unwrap();
    let tracks = library
        .list_tracks(folder.clone(), true, String::new(), 10, 0)
        .await
        .unwrap();
    assert_eq!(tracks.len(), 1);
    assert_eq!(tracks[0].title.as_deref(), Some("Synthetic tone"));
    assert_eq!(tracks[0].duration_ms, Some(2000));
    let original_id = tracks[0].id;
    plugins.unregister(PLUGIN).await.unwrap();
    std::fs::copy(&path, root.join("disabled.ncm")).unwrap();
    library.scan_all().await.unwrap();
    let tracks = library
        .list_tracks(folder, true, String::new(), 10, 0)
        .await
        .unwrap();
    assert_eq!(tracks.len(), 1);
    assert_eq!(tracks[0].id, original_id);
    library.shutdown().await.unwrap();
}

fn prepare_package(payload: &std::path::Path) {
    static HOST: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    let binary = HOST.get_or_init(|| {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let status = std::process::Command::new(env!("CARGO"))
            .current_dir(&root)
            .args(["build", "-p", "stellatune-ncm-host"])
            .status()
            .unwrap();
        assert!(status.success(), "build standalone NCM plugin host");
        root.join("target/debug").join(if cfg!(windows) {
            "stellatune-ncm-host.exe"
        } else {
            "stellatune-ncm-host"
        })
    });
    let source =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../plugins-native/stellatune-plugin-ncm");
    std::fs::create_dir_all(payload.join("bin")).unwrap();
    for name in ["manifest.json", "plugin.mjs"] {
        std::fs::copy(source.join(name), payload.join(name)).unwrap();
    }
    std::fs::copy(
        binary,
        payload.join("bin").join(binary.file_name().unwrap()),
    )
    .unwrap();
}
