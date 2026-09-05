use super::*;
use serde_json::{Value, json};
use std::time::Duration;

#[tokio::test]
async fn host_api_controls_existing_service_without_auth_and_streams_native_edits() {
    let directory = tempfile::tempdir().unwrap();
    let media = directory.path().join("fixture.bin");
    std::fs::write(&media, [250_u8, 250]).unwrap();
    let catalog = PlayerCatalog::open(directory.path().join("player.sqlite"))
        .await
        .unwrap();
    let runtime = test_runtime();
    let service = Arc::new(PlayerService::new(
        catalog,
        runtime.controller(),
        Arc::new(FileLocalResolver {
            path: media,
            resolves: Arc::new(AtomicUsize::new(0)),
        }),
        Arc::new(CountingResolverFactory {
            creates: Arc::new(AtomicUsize::new(0)),
            resolves: Arc::new(AtomicUsize::new(0)),
        }),
    ));
    let plugins = Arc::new(stellatune_plugins::typescript::TypeScriptRuntime::new(
        "unused.mjs",
    ));
    service.start_state_writer();
    let handle = crate::host_api::start(service.clone(), runtime.controller(), plugins)
        .await
        .unwrap();
    let base = handle.base_url();
    assert!(base.starts_with("http://127.0.0.1:"));
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let preflight = client
        .request(reqwest::Method::OPTIONS, format!("{base}/player/commands"))
        .header("origin", "http://localhost:5173")
        .header("access-control-request-method", "POST")
        .send()
        .await
        .unwrap();
    assert_eq!(preflight.status(), 204);
    assert_eq!(preflight.headers()["access-control-allow-origin"], "*");
    let initial: Value = client
        .get(format!("{base}/player/state"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(initial["state"], "idle");
    let mut stream = client
        .get(format!("{base}/player/events"))
        .send()
        .await
        .unwrap();
    let first = stream.chunk().await.unwrap().unwrap();
    assert!(String::from_utf8_lossy(&first).contains("snapshot"));

    let track = service.ensure_local_track(7).await.unwrap();
    let native = service.append_queue(vec![track]).await.unwrap();
    let event = tokio::time::timeout(Duration::from_secs(2), stream.chunk())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(String::from_utf8_lossy(&event).contains("queueChanged"));
    let endpoint = format!("{base}/player/commands");
    let replaced: Value = client
        .post(&endpoint)
        .json(&json!({"command":"replaceQueue", "trackIds":[track.get().to_string()]}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(replaced["items"].as_array().unwrap().len(), 1);
    assert_ne!(
        replaced["items"][0]["itemId"],
        native.items[0].item_id.get().to_string()
    );
    let item = service.queue_snapshot().await.unwrap().items[0].item_id;
    service
        .select_item(
            item,
            SwitchOptions {
                autoplay: false,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let response = client
        .post(&endpoint)
        .json(&json!({"command":"seek", "positionMs":40}))
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success());
    let snapshot: Value = client
        .get(format!("{base}/player/state"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(snapshot["trackId"], track.get().to_string());
    assert_eq!(snapshot["positionMs"], 40);
    assert_eq!(snapshot["durationMs"], 250);
    for body in [
        json!({"command":"appendQueue", "trackIds":[0]}),
        json!({"command":"playTrack", "trackId":"0"}),
        json!({"command":"arbitrary.plugin.action"}),
    ] {
        let response = client.post(&endpoint).json(&body).send().await.unwrap();
        assert_eq!(response.status(), 400);
        assert!(response.json::<Value>().await.unwrap()["code"].is_string());
    }
    let missing = client
        .post(&endpoint)
        .json(&json!({"command":"playTrack","trackId":"99999"}))
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), 404);
    // Shutdown must close live SSE subscribers without waiting for clients to disconnect.
    tokio::time::timeout(Duration::from_secs(3), handle.shutdown())
        .await
        .unwrap();
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn host_provider_commands_validate_capabilities_and_reuse_catalog_identity() {
    let directory = tempfile::tempdir().unwrap();
    let media = directory.path().join("fixture.bin");
    std::fs::write(&media, [250_u8, 250]).unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let mut manifest = stellatune_plugins::typescript::manifest::read_typescript_manifest(
        &root.join("tools/typescript-plugin-runtime/fixtures/manifest.json"),
    )
    .unwrap();
    manifest.runtime.entry = "plugin.mjs".into();
    let plan = json!({"source": {"kind":"file","path":media}, "media":{"codecHint":"bin"}, "capabilities":{"seekable":true}});
    std::fs::write(
        directory.path().join("plugin.mjs"),
        format!("export default {{ invoke() {{ return {plan}; }} }};"),
    )
    .unwrap();
    let plugins = Arc::new(stellatune_plugins::typescript::TypeScriptRuntime::new(
        root.join("tools/typescript-plugin-runtime/runner.mjs"),
    ));
    plugins
        .register(manifest.clone(), directory.path())
        .await
        .unwrap();
    let catalog = PlayerCatalog::open(directory.path().join("player.sqlite"))
        .await
        .unwrap();
    let runtime = test_runtime();
    let service = Arc::new(PlayerService::new(
        catalog.clone(),
        runtime.controller(),
        Arc::new(UnusedLocalResolver),
        Arc::new(crate::runtime::TypeScriptSourceResolverFactory::new(
            plugins.clone(),
        )),
    ));
    let handle = crate::host_api::start(service.clone(), runtime.controller(), plugins.clone())
        .await
        .unwrap();
    plugins.configure_host(handle.base_url(), directory.path().join("data"));
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let endpoint = format!("{}/player/commands", handle.base_url());
    let mut input = json!({"pluginId":manifest.id, "capabilityId":"fixture-source", "providerId":"account", "providerKey":"42"});
    let first: Value = client
        .post(&endpoint)
        .json(&json!({"command":"enqueueProviderTrack","track":input}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let second: Value = client
        .post(&endpoint)
        .json(&json!({"command":"playProviderTrack","track":input}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(first["trackId"], second["trackId"]);
    assert_ne!(first["itemId"], second["itemId"]);
    let from_native = super::super::plugin_tracks::ensure_provider_track(
        &service,
        plugins.clone(),
        &manifest.id,
        "fixture-source",
        "account",
        "42",
    )
    .await
    .unwrap();
    assert_eq!(first["trackId"], from_native.get().to_string());
    let queue = service.queue_snapshot().await.unwrap();
    assert_eq!(queue.items.len(), 2);
    input["capabilityId"] = json!("fixture-search");
    let wrong_kind = client
        .post(&endpoint)
        .json(&json!({"command":"enqueueProviderTrack","track":input}))
        .send()
        .await
        .unwrap();
    assert_eq!(wrong_kind.status(), 400);
    input["capabilityId"] = json!("missing");
    let missing = client
        .post(&endpoint)
        .json(&json!({"command":"enqueueProviderTrack","track":input}))
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), 404);
    assert_eq!(service.queue_snapshot().await.unwrap().items.len(), 2);
    plugins.shutdown().await.unwrap();
    handle.shutdown().await;
    runtime.shutdown().await.unwrap();
}
