use stellatune_plugin_api::StLogLevel;
use stellatune_plugin_api::{STELLATUNE_PLUGIN_API_VERSION, StHostVTable, StStr};

pub(super) fn default_host_vtable() -> StHostVTable {
    extern "C" fn default_host_log(_: *mut core::ffi::c_void, level: StLogLevel, msg: StStr) {
        let text = unsafe { crate::util::ststr_to_string_lossy(msg) };
        match level {
            StLogLevel::Error => tracing::error!(target: "stellatune_plugins::plugin", "{text}"),
            StLogLevel::Warn => tracing::warn!(target: "stellatune_plugins::plugin", "{text}"),
            StLogLevel::Info => tracing::info!(target: "stellatune_plugins::plugin", "{text}"),
            StLogLevel::Debug => tracing::debug!(target: "stellatune_plugins::plugin", "{text}"),
            StLogLevel::Trace => tracing::trace!(target: "stellatune_plugins::plugin", "{text}"),
        }
    }

    StHostVTable {
        api_version: STELLATUNE_PLUGIN_API_VERSION,
        user_data: core::ptr::null_mut(),
        log_utf8: Some(default_host_log),
        get_runtime_root_utf8: None,
        emit_event_json_utf8: None,
        poll_host_event_json_utf8: None,
        send_control_json_utf8: None,
        free_host_str_utf8: None,
    }
}
