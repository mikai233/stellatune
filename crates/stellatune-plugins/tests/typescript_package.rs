use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use stellatune_plugins::typescript::package::{
    discover_typescript_plugins, install_typescript_artifact, uninstall_typescript_plugin,
};

const PLUGIN_ID: &str = "dev.stellatune.fixture.http-source";

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tools/typescript-plugin-runtime/fixtures")
}

fn write_json(path: impl AsRef<Path>, value: &Value) {
    std::fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

fn legacy_installation(plugins: &Path, id: &str) -> PathBuf {
    let root = plugins.join(id);
    std::fs::create_dir_all(root.join("wasm")).unwrap();
    let manifest = json!({
        "schema_version": 1, "api_version": 1,
        "id": id, "name": "Legacy plugin", "version": "0.1.0",
        "components": [{"id": "legacy", "path": "wasm/plugin.wasm"}]
    });
    write_json(root.join("plugin.json"), &manifest);
    let mut receipt_manifest = manifest;
    receipt_manifest["ui"] = Value::Null;
    write_json(
        root.join(".install.json"),
        &json!({"manifest": receipt_manifest, "manifest_rel_path": "plugin.json"}),
    );
    std::fs::write(root.join("wasm/plugin.wasm"), b"legacy component").unwrap();
    std::fs::write(root.join(".ui-config.json"), b"legacy settings").unwrap();
    root
}

fn files(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut result = walkdir::WalkDir::new(root)
        .into_iter()
        .map(Result::unwrap)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| {
            (
                entry.path().strip_prefix(root).unwrap().to_path_buf(),
                std::fs::read(entry.path()).unwrap(),
            )
        })
        .collect::<Vec<_>>();
    result.sort();
    result
}

#[test]
fn legacy_upgrade_preserves_backup_and_data_across_later_updates_and_uninstall() {
    let app = tempfile::tempdir().unwrap();
    let plugins = app.path().join("plugins");
    let old_root = legacy_installation(&plugins, PLUGIN_ID);
    let old_files = files(&old_root);
    let data = app.path().join("plugin-data").join(PLUGIN_ID);
    std::fs::create_dir_all(&data).unwrap();
    std::fs::write(data.join("settings.json"), b"persistent data").unwrap();

    let installed = install_typescript_artifact(&plugins, &fixture()).unwrap();
    assert_eq!(installed.root_dir, old_root);
    assert!(old_root.join(".install-v2.json").is_file());
    assert!(!old_root.join("wasm").exists());
    assert!(!old_root.join(".ui-config.json").exists());
    let backup = std::fs::read_dir(&plugins)
        .unwrap()
        .map(Result::unwrap)
        .find(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with(".legacy-backup-")
        })
        .unwrap()
        .path();
    assert_eq!(files(&backup), old_files);
    assert_eq!(discover_typescript_plugins(&plugins).unwrap().len(), 1);

    install_typescript_artifact(&plugins, &fixture()).unwrap();
    assert_eq!(discover_typescript_plugins(&plugins).unwrap().len(), 1);
    assert_eq!(files(&backup), old_files);
    assert!(
        uninstall_typescript_plugin(&plugins, PLUGIN_ID)
            .unwrap()
            .is_none()
    );
    assert!(discover_typescript_plugins(&plugins).unwrap().is_empty());
    assert_eq!(files(&backup), old_files);
    assert_eq!(
        std::fs::read(data.join("settings.json")).unwrap(),
        b"persistent data"
    );
}

#[test]
fn unknown_or_inconsistent_installations_are_left_untouched() {
    for case in [
        "missing receipt",
        "bad receipt",
        "missing manifest",
        "bad manifest",
        "receipt id",
        "manifest id",
        "receipt schema",
        "manifest schema",
        "version",
        "manifest path",
    ] {
        let plugins = tempfile::tempdir().unwrap();
        let root = legacy_installation(plugins.path(), PLUGIN_ID);
        let receipt_path = root.join(".install.json");
        let manifest_path = root.join("plugin.json");
        let mut receipt: Value =
            serde_json::from_slice(&std::fs::read(&receipt_path).unwrap()).unwrap();
        let mut manifest: Value =
            serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();
        match case {
            "missing receipt" => std::fs::remove_file(&receipt_path).unwrap(),
            "bad receipt" => std::fs::write(&receipt_path, b"invalid").unwrap(),
            "missing manifest" => std::fs::remove_file(&manifest_path).unwrap(),
            "bad manifest" => std::fs::write(&manifest_path, b"invalid").unwrap(),
            "receipt id" | "receipt schema" | "version" | "manifest path" => {
                match case {
                    "receipt id" => receipt["manifest"]["id"] = json!("another.plugin"),
                    "receipt schema" => receipt["manifest"]["schema_version"] = json!(2),
                    "version" => receipt["manifest"]["version"] = json!("different"),
                    _ => receipt["manifest_rel_path"] = json!("../plugin.json"),
                }
                write_json(&receipt_path, &receipt);
            },
            _ => {
                if case == "manifest id" {
                    manifest["id"] = json!("another.plugin");
                } else {
                    manifest["schema_version"] = json!(2);
                }
                write_json(&manifest_path, &manifest);
            },
        }
        let before = files(plugins.path());
        let error = install_typescript_artifact(plugins.path(), &fixture()).unwrap_err();
        assert!(
            error.to_string().contains("unrecognized installation"),
            "{case}: {error}"
        );
        assert_eq!(files(plugins.path()), before, "{case}");
        assert_eq!(
            std::fs::read_dir(plugins.path()).unwrap().count(),
            1,
            "{case}"
        );
    }
}

#[test]
fn invalid_new_artifact_does_not_move_legacy_installation() {
    let plugins = tempfile::tempdir().unwrap();
    legacy_installation(plugins.path(), PLUGIN_ID);
    let before = files(plugins.path());
    let artifact = tempfile::tempdir().unwrap();
    std::fs::write(artifact.path().join("manifest.json"), b"invalid").unwrap();
    assert!(install_typescript_artifact(plugins.path(), artifact.path()).is_err());
    assert_eq!(files(plugins.path()), before);
    assert_eq!(std::fs::read_dir(plugins.path()).unwrap().count(), 1);
}

#[test]
fn packaged_asio_can_replace_an_identified_legacy_installation() {
    let Some(artifact) = std::env::var_os("STELLATUNE_TEST_ASIO_PACKAGE") else {
        return;
    };
    let plugins = tempfile::tempdir().unwrap();
    legacy_installation(plugins.path(), "dev.stellatune.output.asio");
    let installed = install_typescript_artifact(plugins.path(), Path::new(&artifact)).unwrap();
    assert_eq!(installed.manifest.id, "dev.stellatune.output.asio");
    assert!(
        installed
            .root_dir
            .join("bin/stellatune-asio-host.exe")
            .is_file()
    );
    assert_eq!(
        discover_typescript_plugins(plugins.path()).unwrap().len(),
        1
    );
}
