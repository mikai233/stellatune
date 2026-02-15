use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::Duration;

use serde_json::Value;
use stellatune_plugins::install_plugin_from_artifact;
use stellatune_plugins::runtime::handle::PluginRuntimeHandle;

struct FixtureArtifacts {
    v1: PathBuf,
    v2: PathBuf,
}

static TEST_MUTEX: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
static FIXTURES: OnceLock<FixtureArtifacts> = OnceLock::new();

#[tokio::test(flavor = "multi_thread")]
async fn lifecycle_reload_disable_and_gc_with_dynamic_plugins() {
    let _guard = TEST_MUTEX
        .get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await;

    let runtime = PluginRuntimeHandle::new_with_default_host();

    let temp = tempfile::tempdir().expect("create temp dir");
    let plugins_dir = temp.path().join("plugins");
    std::fs::create_dir_all(&plugins_dir).expect("create plugins dir");

    let fixtures = fixture_artifacts();
    let installed = install_plugin_from_artifact(&plugins_dir, &fixtures.v1)
        .expect("install lifecycle fixture v1");

    runtime
        .reload_dir_from_state(&plugins_dir)
        .await
        .expect("load plugin from state");

    let endpoint = runtime
        .bind_decoder_worker_endpoint(&installed.id, "noop")
        .await
        .expect("bind decoder worker endpoint");

    let old_instance = endpoint
        .factory
        .create_instance("{}")
        .expect("create v1 decoder instance");
    assert_eq!(decoder_build_label(&old_instance), "v1");

    std::thread::sleep(Duration::from_millis(50));
    std::fs::copy(&fixtures.v2, &installed.library_path).expect("overwrite plugin library with v2");

    runtime
        .reload_dir_from_state(&plugins_dir)
        .await
        .expect("reload plugin after dll change");

    let new_instance = endpoint
        .factory
        .create_instance("{}")
        .expect("create v2 decoder instance");
    assert_eq!(decoder_build_label(&new_instance), "v2");

    assert_eq!(
        runtime.collect_retired_module_leases_by_refcount().await,
        0,
        "old lease must stay alive while old instance still exists"
    );

    runtime.set_plugin_enabled(&installed.id, false).await;
    let disabled_err = match endpoint.factory.create_instance("{}") {
        Ok(_) => panic!("disabled plugin must reject new instance creation"),
        Err(err) => err,
    };
    assert!(
        disabled_err.to_string().contains("has no active lease"),
        "unexpected disabled error: {disabled_err:#}"
    );

    drop(old_instance);
    let reclaimed = runtime.collect_retired_module_leases_by_refcount().await;
    assert!(
        reclaimed >= 1,
        "expected old lease to be reclaimed after last old instance drop"
    );

    drop(new_instance);
    runtime.set_plugin_enabled(&installed.id, true).await;
    let _ = runtime.unload_plugin(&installed.id).await;
    let _ = runtime.collect_retired_module_leases_by_refcount().await;
    runtime.cleanup_shadow_copies_now().await;

    let _ = runtime.shutdown_and_cleanup().await;
    runtime.cleanup_shadow_copies_now().await;
}

fn decoder_build_label(
    instance: &stellatune_plugins::capabilities::decoder::DecoderInstance,
) -> String {
    let raw = instance
        .get_metadata_json()
        .expect("decoder get_metadata_json")
        .expect("decoder metadata must exist");
    let parsed: Value = serde_json::from_str(&raw).expect("decoder metadata must be valid json");
    parsed
        .get("build")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string()
}

fn fixture_artifacts() -> &'static FixtureArtifacts {
    FIXTURES.get_or_init(|| FixtureArtifacts {
        v1: build_fixture_library(
            "tests/fixtures/lifecycle_plugin_v1/Cargo.toml",
            "lifecycle_plugin_v1",
        ),
        v2: build_fixture_library(
            "tests/fixtures/lifecycle_plugin_v2/Cargo.toml",
            "lifecycle_plugin_v2",
        ),
    })
}

fn build_fixture_library(manifest_rel: &str, crate_name: &str) -> PathBuf {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let manifest_path = crate_root.join(manifest_rel);
    let manifest_dir = manifest_path
        .parent()
        .expect("fixture manifest must have parent dir");

    let status = Command::new(cargo_bin())
        .arg("build")
        .arg("--manifest-path")
        .arg(&manifest_path)
        .current_dir(manifest_dir)
        .status()
        .expect("spawn cargo build for fixture plugin");
    assert!(
        status.success(),
        "fixture build failed: {}",
        manifest_path.display()
    );

    let expected = manifest_dir
        .join("target")
        .join("debug")
        .join(dylib_filename(crate_name));
    if expected.exists() {
        return expected;
    }

    let file_name = dylib_filename(crate_name);
    find_file_recursive(&manifest_dir.join("target").join("debug"), &file_name)
        .unwrap_or_else(|| panic!("cannot locate fixture dylib {}", file_name))
}

fn cargo_bin() -> String {
    std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string())
}

fn dylib_filename(crate_name: &str) -> String {
    let base = crate_name.replace('-', "_");
    match std::env::consts::OS {
        "windows" => format!("{base}.dll"),
        "linux" => format!("lib{base}.so"),
        "macos" => format!("lib{base}.dylib"),
        other => panic!("unsupported test platform: {other}"),
    }
}

fn find_file_recursive(root: &Path, file_name: &str) -> Option<PathBuf> {
    for entry in walkdir::WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        if entry
            .file_name()
            .to_string_lossy()
            .eq_ignore_ascii_case(file_name)
        {
            return Some(entry.path().to_path_buf());
        }
    }
    None
}
