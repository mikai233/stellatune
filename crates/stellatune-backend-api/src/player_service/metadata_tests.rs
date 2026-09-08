use super::*;
use crate::player_service::{identity::TrackId, metadata::TrackPresentation};

#[tokio::test]
async fn provider_projection_preserves_full_unsigned_and_text_keys() {
    let directory = tempfile::tempdir().unwrap();
    let catalog = PlayerCatalog::open(directory.path().join("player.sqlite"))
        .await
        .unwrap();
    let source = catalog
        .ensure_plugin_source(
            ProviderId::new("plugin::source::account").unwrap(),
            resolver_spec("plugin"),
        )
        .await
        .unwrap();
    for (key, expected) in [
        (ProviderTrackKey::Numeric(u64::MAX), u64::MAX.to_string()),
        (
            ProviderTrackKey::Text("00042".to_owned()),
            "00042".to_owned(),
        ),
    ] {
        let track = catalog
            .ensure_track(ProviderTrackIdentity {
                source_instance_id: source,
                provider_key: key,
            })
            .await
            .unwrap();
        let projected = catalog.provider_queue_metadata(&[track]).await.unwrap();
        assert_eq!(projected[&track].provider_key, expected);
        assert_eq!(projected[&track].provider_id, "account");
    }
}

#[tokio::test]
async fn adding_presentation_storage_preserves_existing_catalog_identities() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("player.sqlite");
    let catalog = PlayerCatalog::open(&path).await.unwrap();
    let source = catalog.ensure_local_source().await.unwrap();
    let track = catalog.ensure_local_track(source, 17).await.unwrap();
    sqlx::query("DROP TABLE track_presentation")
        .execute(&catalog.pool)
        .await
        .unwrap();
    catalog.pool.close().await;

    let reopened = PlayerCatalog::open(&path).await.unwrap();
    assert_eq!(reopened.ensure_local_source().await.unwrap(), source);
    assert_eq!(
        reopened.ensure_local_track(source, 17).await.unwrap(),
        track
    );
    let presentation = TrackPresentation {
        title: Some("Preserved identity".to_owned()),
        ..Default::default()
    };
    reopened
        .store_track_presentations(&[(track, presentation.clone())])
        .await
        .unwrap();
    let saved: String =
        sqlx::query_scalar("SELECT presentation_json FROM track_presentation WHERE track_id=?")
            .bind(track.as_i64())
            .fetch_one(&reopened.pool)
            .await
            .unwrap();
    assert_eq!(
        serde_json::from_str::<TrackPresentation>(&saved).unwrap(),
        presentation
    );
}

#[tokio::test]
async fn presentation_batch_rolls_back_when_an_identity_does_not_exist() {
    let directory = tempfile::tempdir().unwrap();
    let catalog = PlayerCatalog::open(directory.path().join("player.sqlite"))
        .await
        .unwrap();
    let source = catalog.ensure_local_source().await.unwrap();
    let track = catalog.ensure_local_track(source, 17).await.unwrap();
    let original = TrackPresentation {
        title: Some("Original".to_owned()),
        ..Default::default()
    };
    catalog
        .store_track_presentations(&[(track, original.clone())])
        .await
        .unwrap();
    let changed = TrackPresentation {
        title: Some("Changed".to_owned()),
        ..Default::default()
    };
    assert!(
        catalog
            .store_track_presentations(&[
                (track, changed.clone()),
                (TrackId::new(99999).unwrap(), changed)
            ])
            .await
            .is_err()
    );
    let saved: String =
        sqlx::query_scalar("SELECT presentation_json FROM track_presentation WHERE track_id=?")
            .bind(track.as_i64())
            .fetch_one(&catalog.pool)
            .await
            .unwrap();
    assert_eq!(
        serde_json::from_str::<TrackPresentation>(&saved).unwrap(),
        original
    );
}
