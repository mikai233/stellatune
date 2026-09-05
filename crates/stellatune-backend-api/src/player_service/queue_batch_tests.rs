use super::*;
use crate::player_service::identity::TrackId;
use std::time::Duration;

#[tokio::test]
async fn library_sized_queue_can_register_project_restore_and_play_past_the_old_limit() {
    let directory = tempfile::tempdir().unwrap();
    let library_path = directory.path().join("library.sqlite");
    let library = stellatune_library::start_library(library_path.to_string_lossy().into_owned())
        .await
        .unwrap();
    let pool = sqlx::SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new().filename(&library_path),
    )
    .await
    .unwrap();
    let fixture = directory.path().join("fixture.bin");
    std::fs::write(&fixture, [255_u8, 10]).unwrap();
    let mut tx = pool.begin().await.unwrap();
    for chunk in (1..=10_000_i64).collect::<Vec<_>>().chunks(200) {
        let mut insert = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
            "INSERT INTO tracks(id,path,mtime_ms,size_bytes) ",
        );
        insert.push_values(chunk, |mut row, id| {
            let path = if *id == 10_000 {
                fixture.to_string_lossy().into_owned()
            } else {
                format!("missing-{id}.bin")
            };
            row.push_bind(*id).push_bind(path).push("0").push("2");
        });
        insert.build().execute(&mut *tx).await.unwrap();
    }
    tx.commit().await.unwrap();
    sqlx::query("DELETE FROM tracks WHERE id=2")
        .execute(&pool)
        .await
        .unwrap();
    let catalog_path = directory.path().join("player.sqlite");
    let catalog = PlayerCatalog::open(&catalog_path).await.unwrap();
    let runtime = test_runtime();
    let service = Arc::new(PlayerService::new(
        catalog.clone(),
        runtime.controller(),
        Arc::new(library.clone()),
        Arc::new(CountingResolverFactory {
            creates: Arc::new(AtomicUsize::new(0)),
            resolves: Arc::new(AtomicUsize::new(0)),
        }),
    ));
    service.start_state_writer();
    let mut ids: Vec<_> = (1..=10_000_i64).collect();
    ids.extend([1, 10_000]);
    let started = std::time::Instant::now();
    let tracks = service.ensure_local_tracks(&ids).await.unwrap();
    assert_eq!(tracks[0], tracks[10_000]);
    assert_eq!(tracks[9_999], tracks[10_001]);
    let queue = service.replace_queue(tracks.clone()).await.unwrap();
    let metadata = service.queue_local_metadata(&tracks).await.unwrap();
    eprintln!(
        "10,002 queue entries: registration, replacement and projection took {:?}",
        started.elapsed()
    );
    assert_eq!(queue.items.len(), ids.len());
    assert_ne!(queue.items[0].item_id, queue.items[10_000].item_id);
    assert_eq!(
        queue
            .items
            .iter()
            .map(|item| item.track_id)
            .collect::<Vec<_>>(),
        tracks
    );
    assert_eq!(metadata[&tracks[1]], (2, None));
    assert_eq!(metadata[&tracks[9_999]], (10_000, Some(fixture)));
    assert_eq!(
        PlayerCatalog::open(&catalog_path)
            .await
            .unwrap()
            .load_state()
            .await
            .unwrap()
            .queue,
        queue.items
    );
    let selected = queue.items.last().unwrap().item_id;
    let mut events = runtime.controller().subscribe_events();
    service
        .select_item(selected, SwitchOptions::default())
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let stellatune_audio::playback::event::PlaybackEvent::TrackChanged { item_id } =
                events.recv().await.unwrap()
            {
                assert_eq!(item_id, selected);
                break;
            }
        }
    })
    .await
    .unwrap();
    service.replace_queue(vec![]).await.unwrap();
    // An in-flight projection retains local identity even after its occurrences disappear.
    assert_eq!(
        service.queue_local_metadata(&tracks).await.unwrap(),
        metadata
    );
    runtime.shutdown().await.unwrap();
    library.shutdown().await.unwrap();
    pool.close().await;
}

#[tokio::test]
async fn invalid_batch_does_not_partially_register_or_replace_a_queue() {
    let directory = tempfile::tempdir().unwrap();
    let catalog = PlayerCatalog::open(directory.path().join("player.sqlite"))
        .await
        .unwrap();
    assert!(catalog.ensure_local_tracks(&[1, 0]).await.is_err());
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM track_catalog")
        .fetch_one(&catalog.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    let ids: Vec<_> = (1..=1_001_i64).collect();
    let mut tracks = catalog.ensure_local_tracks(&ids).await.unwrap();
    let original = catalog.replace_items(&tracks).await.unwrap();
    assert_eq!(catalog.ensure_local_tracks(&ids).await.unwrap(), tracks);
    tracks.push(TrackId::new(i64::MAX as u64).unwrap());
    assert!(catalog.replace_items(&tracks).await.is_err());
    assert!(catalog.append_items(&tracks).await.is_err());
    assert_eq!(catalog.load_state().await.unwrap().queue, original);
}
