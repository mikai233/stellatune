use std::path::Path;
use std::sync::Arc;

use crate::error::Result;
use parking_lot::Mutex;

use crate::executor::WasmPluginController;
use crate::manifest::AbilityKind;
use crate::manifest::{
    AbilitySpec, ComponentSpec, PLUGIN_MANIFEST_FILE_NAME, PluginInstallReceipt,
    WasmPluginManifest, write_receipt,
};
use crate::runtime::model::DesiredPluginState;
use crate::runtime::model::{
    PluginDisableReason, RuntimeCapabilityDescriptor, RuntimePluginInfo,
    RuntimePluginLifecycleState, RuntimePluginTransitionOutcome, RuntimePluginTransitionTrigger,
};
use crate::runtime::service::WasmPluginRuntime;

#[derive(Default)]
struct RecordingLifecycleHost {
    events: Mutex<Vec<String>>,
    fail_uninstall: Mutex<bool>,
}

impl RecordingLifecycleHost {
    fn set_fail_uninstall(&self, fail: bool) {
        *self.fail_uninstall.lock() = fail;
    }

    fn events(&self) -> Vec<String> {
        self.events.lock().clone()
    }
}

impl WasmPluginController for RecordingLifecycleHost {
    fn install_plugin(
        &self,
        plugin: &RuntimePluginInfo,
        _capabilities: &[RuntimeCapabilityDescriptor],
    ) -> Result<()> {
        self.events.lock().push(format!("install:{}", plugin.id));
        Ok(())
    }

    fn uninstall_plugin(&self, plugin_id: &str, reason: PluginDisableReason) -> Result<()> {
        if *self.fail_uninstall.lock() {
            return Err(crate::op_error!(
                "forced uninstall failure for plugin `{}`",
                plugin_id
            ));
        }
        self.events
            .lock()
            .push(format!("uninstall:{plugin_id}:{reason:?}"));
        Ok(())
    }

    fn dispatch_directive(
        &self,
        _plugin_id: &str,
        _directive: crate::runtime::model::RuntimePluginDirective,
    ) -> Result<()> {
        Ok(())
    }

    fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}

fn install_test_plugin(plugins_dir: &Path, plugin_id: &str, version: &str) -> Result<()> {
    install_test_plugin_with_ability(
        plugins_dir,
        plugin_id,
        version,
        AbilityKind::OutputSink,
        "audio-plugin",
        "test.sink",
    )
}

fn install_test_plugin_with_ability(
    plugins_dir: &Path,
    plugin_id: &str,
    version: &str,
    ability_kind: AbilityKind,
    world: &str,
    type_id: &str,
) -> Result<()> {
    let plugin_root = plugins_dir.join(plugin_id);
    std::fs::create_dir_all(&plugin_root)?;

    let component_rel_path = "component.wasm";
    std::fs::write(plugin_root.join(component_rel_path), b"\0asm-test")?;

    let manifest = WasmPluginManifest {
        schema_version: 1,
        id: plugin_id.to_string(),
        name: format!("Test {plugin_id}"),
        version: version.to_string(),
        api_version: 1,
        components: vec![ComponentSpec {
            id: "main".to_string(),
            path: component_rel_path.to_string(),
            world: world.to_string(),
            abilities: vec![AbilitySpec {
                kind: ability_kind,
                type_id: type_id.to_string(),
                display_name: None,
                config_schema_json: None,
                default_config_json: None,
                decoder: None,
            }],
        }],
    };
    let manifest_text = serde_json::to_string_pretty(&manifest)?;
    std::fs::write(plugin_root.join(PLUGIN_MANIFEST_FILE_NAME), manifest_text)?;

    let receipt = PluginInstallReceipt {
        manifest,
        manifest_rel_path: PLUGIN_MANIFEST_FILE_NAME.to_string(),
    };
    write_receipt(&plugin_root, &receipt)?;
    Ok(())
}

fn update_plugin_version(plugins_dir: &Path, plugin_id: &str, version: &str) -> Result<()> {
    let manifest_path = plugins_dir.join(plugin_id).join(PLUGIN_MANIFEST_FILE_NAME);
    let raw = std::fs::read_to_string(&manifest_path)?;
    let mut manifest: WasmPluginManifest = serde_json::from_str(&raw)?;
    manifest.version = version.to_string();
    let text = serde_json::to_string_pretty(&manifest)?;
    std::fs::write(&manifest_path, text)?;
    Ok(())
}

#[test]
fn sync_activates_new_plugin() {
    let temp = tempfile::tempdir().expect("create tempdir");
    let plugins_dir = temp.path().join("plugins");
    std::fs::create_dir_all(&plugins_dir).expect("create plugins dir");
    install_test_plugin(&plugins_dir, "demo", "1.0.0").expect("install test plugin");

    let host = Arc::new(RecordingLifecycleHost::default());
    let runtime = WasmPluginRuntime::new(host.clone());
    let report = runtime.sync_plugins(&plugins_dir).expect("sync runtime");

    assert_eq!(report.active_plugins.len(), 1);
    assert!(report.transitions.iter().any(|transition| {
        transition.trigger == RuntimePluginTransitionTrigger::LoadNew
            && transition.outcome == RuntimePluginTransitionOutcome::Applied
    }));
    assert_eq!(host.events(), vec!["install:demo".to_string()]);
}

#[test]
fn disable_desired_state_deactivates_plugin() {
    let temp = tempfile::tempdir().expect("create tempdir");
    let plugins_dir = temp.path().join("plugins");
    std::fs::create_dir_all(&plugins_dir).expect("create plugins dir");
    install_test_plugin(&plugins_dir, "demo", "1.0.0").expect("install test plugin");

    let host = Arc::new(RecordingLifecycleHost::default());
    let runtime = WasmPluginRuntime::new(host.clone());
    runtime.sync_plugins(&plugins_dir).expect("first sync");
    runtime
        .set_desired_state("demo", DesiredPluginState::Disabled)
        .expect("disable plugin");

    let report = runtime.sync_plugins(&plugins_dir).expect("second sync");
    assert!(report.active_plugins.is_empty());
    assert!(report.transitions.iter().any(|transition| {
        transition.trigger == RuntimePluginTransitionTrigger::DisableRequested
            && transition.outcome != RuntimePluginTransitionOutcome::Failed
    }));
    assert_eq!(
        host.events(),
        vec![
            "install:demo".to_string(),
            "uninstall:demo:HostDisable".to_string()
        ]
    );
}

#[test]
fn manifest_change_triggers_reload_lifecycle() {
    let temp = tempfile::tempdir().expect("create tempdir");
    let plugins_dir = temp.path().join("plugins");
    std::fs::create_dir_all(&plugins_dir).expect("create plugins dir");
    install_test_plugin(&plugins_dir, "demo", "1.0.0").expect("install test plugin");

    let host = Arc::new(RecordingLifecycleHost::default());
    let runtime = WasmPluginRuntime::new(host.clone());
    runtime.sync_plugins(&plugins_dir).expect("first sync");
    update_plugin_version(&plugins_dir, "demo", "2.0.0").expect("update version");

    let report = runtime.sync_plugins(&plugins_dir).expect("reload sync");
    assert!(report.transitions.iter().any(|transition| {
        transition.trigger == RuntimePluginTransitionTrigger::ReloadChanged
            && transition.outcome == RuntimePluginTransitionOutcome::Applied
    }));
    assert_eq!(
        host.events(),
        vec![
            "install:demo".to_string(),
            "uninstall:demo:Reload".to_string(),
            "install:demo".to_string()
        ]
    );
}

#[test]
fn plugin_removal_uses_unload_reason() {
    let temp = tempfile::tempdir().expect("create tempdir");
    let plugins_dir = temp.path().join("plugins");
    std::fs::create_dir_all(&plugins_dir).expect("create plugins dir");
    install_test_plugin(&plugins_dir, "demo", "1.0.0").expect("install test plugin");

    let host = Arc::new(RecordingLifecycleHost::default());
    let runtime = WasmPluginRuntime::new(host.clone());
    runtime.sync_plugins(&plugins_dir).expect("first sync");

    std::fs::remove_dir_all(plugins_dir.join("demo")).expect("remove plugin dir");
    let report = runtime.sync_plugins(&plugins_dir).expect("second sync");
    assert!(report.transitions.iter().any(|transition| {
        transition.trigger == RuntimePluginTransitionTrigger::RemovedFromDisk
            && transition.outcome == RuntimePluginTransitionOutcome::Applied
    }));
    assert_eq!(
        host.events(),
        vec![
            "install:demo".to_string(),
            "uninstall:demo:Unload".to_string()
        ]
    );
}

#[test]
fn shutdown_uses_shutdown_reason() {
    let temp = tempfile::tempdir().expect("create tempdir");
    let plugins_dir = temp.path().join("plugins");
    std::fs::create_dir_all(&plugins_dir).expect("create plugins dir");
    install_test_plugin(&plugins_dir, "demo", "1.0.0").expect("install test plugin");

    let host = Arc::new(RecordingLifecycleHost::default());
    let runtime = WasmPluginRuntime::new(host.clone());
    runtime.sync_plugins(&plugins_dir).expect("first sync");
    runtime.shutdown().expect("shutdown runtime");

    assert_eq!(
        host.events(),
        vec![
            "install:demo".to_string(),
            "uninstall:demo:Shutdown".to_string()
        ]
    );
}

#[test]
fn dropping_clone_does_not_shutdown_runtime() {
    let temp = tempfile::tempdir().expect("create tempdir");
    let plugins_dir = temp.path().join("plugins");
    std::fs::create_dir_all(&plugins_dir).expect("create plugins dir");
    install_test_plugin(&plugins_dir, "demo", "1.0.0").expect("install test plugin");

    let host = Arc::new(RecordingLifecycleHost::default());
    let runtime = WasmPluginRuntime::new(host.clone());
    runtime.sync_plugins(&plugins_dir).expect("first sync");

    let runtime_clone = runtime.clone();
    drop(runtime_clone);

    assert_eq!(host.events(), vec!["install:demo".to_string()]);
}

#[test]
fn failed_reload_keeps_previous_plugin_active_and_records_error() {
    let temp = tempfile::tempdir().expect("create tempdir");
    let plugins_dir = temp.path().join("plugins");
    std::fs::create_dir_all(&plugins_dir).expect("create plugins dir");
    install_test_plugin(&plugins_dir, "demo", "1.0.0").expect("install test plugin");

    let host = Arc::new(RecordingLifecycleHost::default());
    let runtime = WasmPluginRuntime::new(host.clone());
    runtime.sync_plugins(&plugins_dir).expect("first sync");

    update_plugin_version(&plugins_dir, "demo", "2.0.0").expect("update version");
    host.set_fail_uninstall(true);

    let report = runtime
        .sync_plugins(&plugins_dir)
        .expect("reload sync with forced failure");
    assert_eq!(report.active_plugins.len(), 1);
    assert!(report.transitions.iter().any(|transition| {
        transition.trigger == RuntimePluginTransitionTrigger::ReloadChanged
            && transition.outcome == RuntimePluginTransitionOutcome::Failed
    }));

    let status = report
        .plugin_statuses
        .iter()
        .find(|status| status.plugin_id == "demo")
        .expect("plugin status for demo");
    assert_eq!(status.lifecycle_state, RuntimePluginLifecycleState::Active);
    assert!(
        status
            .last_error
            .as_deref()
            .is_some_and(|value| !value.is_empty())
    );
}
