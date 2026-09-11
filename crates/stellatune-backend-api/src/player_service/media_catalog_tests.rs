use super::*;
use crate::media_catalog::MediaCatalogService;
use crate::player_service::{identity::TrackId, source::TrackOrigin};
use std::time::Duration;
use stellatune_library::catalog::{CatalogQuery, CatalogSort, MediaKind};
use stellatune_plugins::typescript::{TypeScriptRuntime, manifest::read_typescript_manifest};

#[tokio::test]
async fn media_catalog_instances_pagination_identity_restart_and_cancellation() {
    let directory = tempfile::tempdir().unwrap();
    let db = directory.path().join("library.sqlite");
    let local = stellatune_library::start_library(db.to_string_lossy().into_owned())
        .await
        .unwrap();
    let runtime = test_runtime();
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tools/typescript-plugin-runtime");
    let plugins = Arc::new(TypeScriptRuntime::new(root.join("runner.mjs")));
    plugins.configure_host(
        "http://127.0.0.1:1".into(),
        directory.path().join("plugin-data"),
    );
    let package = root.join("media-library-fixture");
    let manifest = read_typescript_manifest(&package.join("manifest.json")).unwrap();
    plugins.register(manifest.clone(), &package).await.unwrap();
    let player = Arc::new(PlayerService::new(
        PlayerCatalog::open(&db).await.unwrap(),
        runtime.controller(),
        Arc::new(local.clone()),
        Arc::new(crate::runtime::TypeScriptSourceResolverFactory::new(
            plugins.clone(),
        )),
    ));
    let catalog = Arc::new(MediaCatalogService::new(
        local.catalog().clone(),
        player.clone(),
        plugins.clone(),
    ));
    let sources = catalog.list_sources().await.unwrap();
    assert_eq!(sources.len(), 3);
    let a = sources.iter().find(|s| s.name == "server-a").unwrap();
    let b = sources.iter().find(|s| s.name == "server-b").unwrap();
    assert_ne!(a.id, b.id);
    let query = |source: &str, kind| CatalogQuery {
        source_instance_id: source.into(),
        kind,
        parent: None,
        search: String::new(),
        sort: CatalogSort::Default,
        cursor: None,
        limit: 200,
    };
    for kind in [
        MediaKind::Album,
        MediaKind::Artist,
        MediaKind::Folder,
        MediaKind::Playlist,
    ] {
        let mut q = query(&a.id, kind);
        let page = catalog.browse(q.clone()).await.unwrap();
        assert_eq!(page.items.len(), 200);
        assert_eq!(
            catalog
                .detail(page.items[0].reference.clone())
                .await
                .unwrap()
                .reference,
            page.items[0].reference
        );
        q.cursor = page.next_cursor;
        let page = catalog.browse(q).await.unwrap();
        assert_eq!(page.items.len(), 5);
        assert!(page.next_cursor.is_none());
    }
    let tracks = catalog
        .collect_tracks(query(&a.id, MediaKind::Track), "all".into())
        .await
        .unwrap();
    assert_eq!(tracks.len(), 405);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM track_catalog")
        .fetch_one(&player.catalog.pool)
        .await
        .unwrap();
    assert_eq!(count, 0, "browsing must not register playback tracks");
    let other = catalog
        .browse(query(&b.id, MediaKind::Track))
        .await
        .unwrap()
        .items
        .remove(0);
    assert_eq!(tracks[0].reference.id, "001");
    let ids = catalog
        .prepare_tracks(vec![tracks[0].clone(), other])
        .await
        .unwrap();
    assert_ne!(ids[0], ids[1]);
    let host_id = crate::media_catalog::ensure_catalog_provider_track(
        &player,
        plugins.clone(),
        &manifest.id,
        "library",
        "source",
        "server-a",
        "001",
    )
    .await
    .unwrap();
    assert_eq!(ids[0], host_id.get());
    for (id, name) in [(ids[0], "server-a"), (ids[1], "server-b")] {
        let track = player
            .catalog
            .track(TrackId::new(id).unwrap())
            .await
            .unwrap();
        let source = player.catalog.source(track.source).await.unwrap();
        let spec = source.resolver.as_ref().unwrap();
        let resolver = crate::runtime::TypeScriptSourceResolver::new(
            plugins.clone(),
            &manifest.id,
            &spec.capability_id,
        );
        let TrackOrigin::Provider(key) = track.origin else {
            panic!("provider expected")
        };
        let resolved = resolver.resolve(&source, &key).await.unwrap();
        let ResolvedSourceSpec::Http { url, headers, .. } = resolved else {
            panic!("HTTP expected")
        };
        assert!(url.ends_with(&format!("/{name}/001")));
        assert_eq!(headers["X-Library"], name);
    }
    catalog.cancel_collection("early-cancel".into()).await;
    assert!(
        catalog
            .collect_tracks(query(&a.id, MediaKind::Track), "early-cancel".into())
            .await
            .unwrap_err()
            .to_string()
            .contains("cancelled")
    );
    let slow_catalog = catalog.clone();
    let mut slow = query(&a.id, MediaKind::Track);
    slow.search = "slow".into();
    let job =
        tokio::spawn(async move { slow_catalog.collect_tracks(slow, "cancel-me".into()).await });
    tokio::time::sleep(Duration::from_millis(30)).await;
    catalog.cancel_collection("cancel-me".into()).await;
    assert!(
        job.await
            .unwrap()
            .unwrap_err()
            .to_string()
            .contains("cancelled")
    );
    plugins.unregister(&manifest.id).await.unwrap();
    assert!(
        catalog
            .list_sources()
            .await
            .unwrap()
            .iter()
            .filter(|s| !s.local)
            .all(|s| !s.available)
    );
    assert!(
        catalog
            .browse(query(&a.id, MediaKind::Track))
            .await
            .is_err()
    );
    plugins.register(manifest, &package).await.unwrap();
    let reopened = PlayerCatalog::open(&db).await.unwrap();
    assert_eq!(
        reopened
            .track(TrackId::new(ids[0]).unwrap())
            .await
            .unwrap()
            .source
            .get()
            .to_string(),
        a.id
    );
    assert_eq!(
        catalog
            .list_sources()
            .await
            .unwrap()
            .iter()
            .find(|s| s.name == "server-a")
            .unwrap()
            .id,
        a.id
    );
    plugins.shutdown().await.unwrap();
    local.shutdown().await.unwrap();
    runtime.shutdown().await.unwrap();
}
