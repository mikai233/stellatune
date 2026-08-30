use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use serde_json::{Value, json};
use stellatune_plugins::typescript::manifest::{
    ManifestV2Error, read_typescript_manifest, validate_typescript_manifest,
};
use stellatune_plugins::typescript::package::{
    discover_typescript_plugins, install_typescript_artifact, uninstall_typescript_plugin,
};
use stellatune_plugins::typescript::protocol::{SourceLocatorDto, SourcePlanDto};
use stellatune_plugins::typescript::{
    PluginProcessConfig, PluginProcessHandle, PluginRuntimeError, TypeScriptRuntime,
};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("plugin crate must be under repository/crates")
        .to_path_buf()
}

fn fixture_paths() -> (PathBuf, PathBuf, PathBuf) {
    let root = repository_root();
    let fixture = root.join("tools/typescript-plugin-runtime/fixtures");
    (
        fixture.join("manifest.json"),
        fixture.join("http-source-plugin.mjs"),
        root.join("tools/typescript-plugin-runtime/runner.mjs"),
    )
}

#[test]
fn manifest_v2_fixture_has_no_permissions_and_validates_bundle() {
    let (manifest_path, _, _) = fixture_paths();
    let manifest =
        read_typescript_manifest(&manifest_path).expect("fixture manifest must validate");
    assert_eq!(manifest.manifest_version, 2);
    assert_eq!(manifest.capabilities.len(), 2);
    validate_typescript_manifest(&manifest, manifest_path.parent().unwrap()).unwrap();
}

#[tokio::test]
async fn first_party_netease_bundle_uses_control_rpc_and_returns_no_media_bytes() {
    let root = repository_root();
    let source = root.join("crates/plugins-native/stellatune-plugin-netease");
    let package = tempfile::tempdir().unwrap();
    for file in ["manifest.json", "plugin.mjs", "source-config.schema.json"] {
        std::fs::copy(source.join(file), package.path().join(file)).unwrap();
    }
    std::fs::create_dir(package.path().join("ui")).unwrap();
    std::fs::write(package.path().join("ui/index.html"), "<!doctype html>").unwrap();
    let manifest = read_typescript_manifest(&package.path().join("manifest.json")).unwrap();
    assert_eq!(manifest.manifest_version, 2);
    assert_eq!(manifest.capabilities.len(), 4);

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        for index in 0..2 {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 2048];
            let read = stream.read(&mut request).unwrap();
            let request = String::from_utf8_lossy(&request[..read]);
            let body = if index == 0 {
                assert!(request.starts_with("GET /health"));
                r#"{"ok":true}"#
            } else {
                assert!(request.starts_with("GET /v1/search?"));
                r#"{"items":[{"song_id":42,"title":"fixture","ext_hint":"flac"}]}"#
            };
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
        }
    });

    let runtime = TypeScriptRuntime::new(root.join("tools/typescript-plugin-runtime/runner.mjs"));
    runtime.register(manifest, package.path()).await.unwrap();
    let result = runtime
        .invoke(
            "dev.stellatune.source.netease",
            "netease-search",
            None,
            "list-items",
            json!({
                "action": "search",
                "keywords": "fixture",
                "config": { "sidecarBaseUrl": format!("http://{address}") }
            }),
            None,
        )
        .await
        .unwrap();
    assert_eq!(result.value[0]["track"]["song_id"], 42);
    assert!(result.value.to_string().find("mediaBytes").is_none());
    runtime.shutdown().await.unwrap();
    server.join().unwrap();
}

#[test]
fn manifest_v2_rejects_permissions_as_an_unknown_field() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("plugin.mjs"), "export default {};").unwrap();
    let manifest_path = temp.path().join("manifest.json");
    std::fs::write(
        &manifest_path,
        serde_json::to_vec(&json!({
            "manifest_version": 2,
            "id": "dev.stellatune.fixture.invalid",
            "name": "Invalid",
            "version": "0.1.0",
            "permissions": ["network"],
            "runtime": {
                "kind": "typescript",
                "entry": "plugin.mjs",
                "api_version": 2,
                "protocol": "stellatune-capability-rpc/1"
            },
            "capabilities": [{
                "id": "source",
                "kind": "source-resolver",
                "execution_class": "control",
                "display_name": "Source"
            }]
        }))
        .unwrap(),
    )
    .unwrap();
    assert!(matches!(
        read_typescript_manifest(&manifest_path),
        Err(ManifestV2Error::Parse { .. })
    ));
}

#[test]
fn v2_package_installs_updates_discovers_and_uninstalls_without_starting_node() {
    let root = repository_root();
    let fixture = root.join("tools/typescript-plugin-runtime/fixtures");
    let plugins = tempfile::tempdir().unwrap();

    let installed = install_typescript_artifact(plugins.path(), &fixture).unwrap();
    assert_eq!(installed.manifest.version, "0.1.0");
    assert_eq!(
        discover_typescript_plugins(plugins.path()).unwrap().len(),
        1
    );

    let update = tempfile::tempdir().unwrap();
    std::fs::copy(
        fixture.join("http-source-plugin.mjs"),
        update.path().join("http-source-plugin.mjs"),
    )
    .unwrap();
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(fixture.join("manifest.json")).unwrap()).unwrap();
    manifest["version"] = json!("0.2.0");
    std::fs::write(
        update.path().join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    let updated = install_typescript_artifact(plugins.path(), update.path()).unwrap();
    assert_eq!(updated.manifest.version, "0.2.0");
    assert_eq!(
        discover_typescript_plugins(plugins.path()).unwrap()[0]
            .manifest
            .version,
        "0.2.0"
    );

    assert!(
        uninstall_typescript_plugin(plugins.path(), "dev.stellatune.fixture.http-source")
            .unwrap()
            .is_none()
    );
    assert!(
        discover_typescript_plugins(plugins.path())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn v2_package_rejects_node_modules_content() {
    let root = repository_root();
    let fixture = root.join("tools/typescript-plugin-runtime/fixtures");
    let artifact = tempfile::tempdir().unwrap();
    std::fs::copy(
        fixture.join("http-source-plugin.mjs"),
        artifact.path().join("http-source-plugin.mjs"),
    )
    .unwrap();
    std::fs::copy(
        fixture.join("manifest.json"),
        artifact.path().join("manifest.json"),
    )
    .unwrap();
    std::fs::create_dir(artifact.path().join("node_modules")).unwrap();
    std::fs::write(
        artifact.path().join("node_modules/dependency.mjs"),
        "export default {};",
    )
    .unwrap();
    let plugins = tempfile::tempdir().unwrap();
    assert!(install_typescript_artifact(plugins.path(), artifact.path()).is_err());
}

#[tokio::test]
async fn one_lazy_process_serves_multiple_capabilities_and_restarts_after_idle() {
    let (_, entry, runner) = fixture_paths();
    let mut config = PluginProcessConfig::new("dev.stellatune.fixture.http-source", entry, runner);
    config.request_timeout = Duration::from_secs(3);
    config.idle_timeout = Duration::from_millis(150);
    let handle = PluginProcessHandle::spawn(config);

    assert!(!handle.snapshot().await.unwrap().running);
    let resolved = handle
        .invoke(
            "fixture-source",
            None,
            "resolve",
            json!({"url": "https://example.test/media.flac"}),
            None,
        )
        .await
        .unwrap();
    let source_plan: SourcePlanDto = serde_json::from_value(resolved.value).unwrap();
    assert!(matches!(
        source_plan.source,
        SourceLocatorDto::Http { ref url, .. } if url == "https://example.test/media.flac"
    ));
    let first_snapshot = handle.snapshot().await.unwrap();
    assert!(first_snapshot.running);

    let echoed = handle
        .invoke(
            "fixture-search",
            Some("search-session".to_string()),
            "echo",
            json!({"query": "same process"}),
            Some(resolved.generation),
        )
        .await
        .unwrap();
    assert_eq!(echoed.generation, resolved.generation);
    assert_eq!(
        handle.snapshot().await.unwrap().process_id,
        first_snapshot.process_id
    );

    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(!handle.snapshot().await.unwrap().running);
    assert!(matches!(
        handle
            .invoke(
                "fixture-search",
                None,
                "echo",
                Value::Null,
                Some(resolved.generation)
            )
            .await,
        Err(PluginRuntimeError::GenerationMismatch { .. })
    ));

    let restarted = handle
        .invoke(
            "fixture-search",
            None,
            "echo",
            json!({"after": "idle"}),
            None,
        )
        .await
        .unwrap();
    assert!(restarted.generation > resolved.generation);
    handle.shutdown().await.unwrap();
}
