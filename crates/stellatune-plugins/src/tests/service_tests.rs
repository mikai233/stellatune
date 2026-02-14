use super::{PluginRuntimeService, STELLATUNE_PLUGIN_API_VERSION, StHostVTable};

fn test_host() -> StHostVTable {
    StHostVTable {
        api_version: STELLATUNE_PLUGIN_API_VERSION,
        user_data: core::ptr::null_mut(),
        log_utf8: None,
        get_runtime_root_utf8: None,
        emit_event_json_utf8: None,
        begin_poll_host_event_json_utf8: None,
        begin_send_control_json_utf8: None,
        free_host_str_utf8: None,
    }
}

#[test]
fn disable_unknown_plugin_returns_false() {
    let mut svc = PluginRuntimeService::new(test_host());
    assert!(!svc.disable_plugin_slot("dev.test.plugin"));
}

#[test]
fn reclaim_retired_without_modules_is_noop() {
    let mut svc = PluginRuntimeService::new(test_host());
    assert_eq!(svc.collect_retired_module_leases_by_refcount(), 0);
}
