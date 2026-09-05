use super::*;
use stellatune_audio::playback::event::PlaybackEvent;
use tokio::{
    sync::Semaphore,
    time::{Duration, timeout},
};

struct GatedLocalResolver {
    path: PathBuf,
    entered: Arc<Semaphore>,
    release: Arc<Semaphore>,
}

#[async_trait]
impl LocalTrackResolver for GatedLocalResolver {
    async fn resolve_path(&self, id: i64) -> Result<PathBuf, PlayerServiceError> {
        if id == 2 {
            self.entered.add_permits(1);
            self.release.acquire().await.unwrap().forget();
        }
        Ok(self.path.clone())
    }
}

fn service(
    catalog: PlayerCatalog,
    runtime: &PlaybackRuntime,
    local: Arc<dyn LocalTrackResolver>,
) -> Arc<PlayerService> {
    let service = Arc::new(PlayerService::new(
        catalog,
        runtime.controller(),
        local,
        Arc::new(CountingResolverFactory {
            creates: Arc::new(AtomicUsize::new(0)),
            resolves: Arc::new(AtomicUsize::new(0)),
        }),
    ));
    service.start_state_writer();
    service
}

#[tokio::test]
async fn natural_playback_replenishes_successors_and_preserves_duplicate_occurrences() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("fixture.bin");
    std::fs::write(&path, [255_u8, 10]).unwrap();
    let catalog = PlayerCatalog::open(directory.path().join("player.sqlite"))
        .await
        .unwrap();
    let runtime = test_runtime();
    let resolves = Arc::new(AtomicUsize::new(0));
    let service = service(
        catalog,
        &runtime,
        Arc::new(FileLocalResolver {
            path,
            resolves: Arc::clone(&resolves),
        }),
    );
    let track = service.ensure_local_track(1).await.unwrap();
    let queue = service
        .replace_queue(vec![track, track, track])
        .await
        .unwrap();
    let expected: Vec<_> = queue.items.iter().map(|item| item.item_id).collect();
    assert_ne!(expected[0], expected[1]);
    assert_ne!(expected[1], expected[2]);
    let mut events = runtime.controller().subscribe_events();
    service
        .select_item(expected[0], SwitchOptions::default())
        .await
        .unwrap();
    let actual = timeout(Duration::from_secs(5), async {
        let mut actual = vec![];
        loop {
            match events.recv().await.unwrap() {
                PlaybackEvent::TrackChanged { item_id } => actual.push(item_id),
                PlaybackEvent::PlaybackEnded { .. } => break actual,
                _ => {},
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(actual, expected);
    assert_eq!(resolves.load(Ordering::SeqCst), 3);
    assert_eq!(service.queue_snapshot().await.unwrap().items, queue.items);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn rapid_next_uses_requested_cursor_while_resolution_is_slow() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("fixture.bin");
    std::fs::write(&path, [255_u8, 10]).unwrap();
    let runtime = test_runtime();
    let entered = Arc::new(Semaphore::new(0));
    let service = service(
        PlayerCatalog::open(directory.path().join("player.sqlite"))
            .await
            .unwrap(),
        &runtime,
        Arc::new(GatedLocalResolver {
            path,
            entered: Arc::clone(&entered),
            release: Arc::new(Semaphore::new(0)),
        }),
    );
    let mut tracks = vec![];
    for id in 1..=3 {
        tracks.push(service.ensure_local_track(id).await.unwrap());
    }
    let queue = service.replace_queue(tracks).await.unwrap();
    service
        .select_item(
            queue.items[0].item_id,
            SwitchOptions {
                autoplay: false,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    timeout(Duration::from_secs(2), entered.acquire())
        .await
        .unwrap()
        .unwrap()
        .forget();
    let mut events = runtime.controller().subscribe_events();
    let first = {
        let service = Arc::clone(&service);
        tokio::spawn(async move { service.next().await })
    };
    timeout(Duration::from_secs(2), entered.acquire())
        .await
        .unwrap()
        .unwrap()
        .forget();
    service.next().await.unwrap();
    assert!(matches!(
        first.await.unwrap(),
        Err(PlayerServiceError::Superseded)
    ));
    timeout(Duration::from_secs(2), async {
        loop {
            if let PlaybackEvent::TrackChanged { item_id } = events.recv().await.unwrap() {
                assert_eq!(item_id, queue.items[2].item_id);
                break;
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(
        service.queue_snapshot().await.unwrap().requested_item_id,
        Some(queue.items[2].item_id)
    );
    assert_eq!(service.queue_snapshot().await.unwrap().items.len(), 3);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn removing_a_preparing_successor_preserves_other_ids_and_cancels_old_resolution() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("fixture.bin");
    std::fs::write(&path, [255_u8, 10]).unwrap();
    let runtime = test_runtime();
    let entered = Arc::new(Semaphore::new(0));
    let release = Arc::new(Semaphore::new(0));
    let service = service(
        PlayerCatalog::open(directory.path().join("player.sqlite"))
            .await
            .unwrap(),
        &runtime,
        Arc::new(GatedLocalResolver {
            path,
            entered: Arc::clone(&entered),
            release: Arc::clone(&release),
        }),
    );
    let mut tracks = vec![];
    for id in 1..=3 {
        tracks.push(service.ensure_local_track(id).await.unwrap());
    }
    let queue = service.replace_queue(tracks).await.unwrap();
    service
        .select_item(
            queue.items[0].item_id,
            SwitchOptions {
                autoplay: false,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    timeout(Duration::from_secs(2), entered.acquire())
        .await
        .unwrap()
        .unwrap()
        .forget();
    let after = service
        .remove_queue_items(vec![queue.items[1].item_id])
        .await
        .unwrap();
    assert_eq!(
        after.items,
        vec![queue.items[0].clone(), queue.items[2].clone()]
    );
    release.add_permits(4);
    service.next().await.unwrap();
    assert_eq!(
        service.queue_snapshot().await.unwrap().requested_item_id,
        Some(queue.items[2].item_id)
    );
    service.stop().await.unwrap();
    assert_eq!(
        runtime.controller().snapshot().await.unwrap().state,
        PlaybackState::Idle
    );
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn invalid_queue_replacement_retains_the_previous_queue() {
    let directory = tempfile::tempdir().unwrap();
    let catalog = PlayerCatalog::open(directory.path().join("player.sqlite"))
        .await
        .unwrap();
    let source = catalog.ensure_local_source().await.unwrap();
    let good = catalog.ensure_local_track(source, 1).await.unwrap();
    let bad = catalog.ensure_local_track(source, 2).await.unwrap();
    let original = catalog.replace_items(&[good, good]).await.unwrap();
    catalog.tombstone_track(bad).await.unwrap();
    assert!(catalog.replace_items(&[good, bad]).await.is_err());
    assert_eq!(catalog.load_state().await.unwrap().queue, original);
    let replacement = catalog.replace_items(&[good]).await.unwrap();
    assert!(replacement[0].item_id.get() > original[1].item_id.get());
}
