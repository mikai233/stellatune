#[doc(hidden)]
#[macro_export]
macro_rules! __st_opt_create_cb {
    () => {
        None
    };
    ($f:path) => {
        Some($f)
    };
}

/// Minimal module exporter.
///
/// This macro intentionally provides a narrow bootstrap surface so migration can proceed
/// incrementally. Capability-specific export macros can be layered on top later.
#[macro_export]
macro_rules! export_plugin_minimal {
    (
        metadata_json_utf8: $metadata_json_utf8:path,
        capability_count: $capability_count:path,
        capability_get: $capability_get:path
        $(, begin_create_decoder_instance: $begin_create_decoder_instance:path)?
        $(, begin_create_dsp_instance: $begin_create_dsp_instance:path)?
        $(, begin_create_source_catalog_instance: $begin_create_source_catalog_instance:path)?
        $(, begin_create_lyrics_provider_instance: $begin_create_lyrics_provider_instance:path)?
        $(, begin_create_output_sink_instance: $begin_create_output_sink_instance:path)?
        $(, begin_quiesce: $begin_quiesce:path)?
        $(, begin_shutdown: $begin_shutdown:path)?
        $(,)?
    ) => {
        static __ST_PLUGIN_MODULE: stellatune_plugin_api::StPluginModule =
            stellatune_plugin_api::StPluginModule {
                api_version: stellatune_plugin_api::STELLATUNE_PLUGIN_API_VERSION,
                plugin_version: stellatune_plugin_api::StVersion {
                    major: 0,
                    minor: 1,
                    patch: 0,
                    reserved: 0,
                },
                plugin_free: Some($crate::plugin_free),
                metadata_json_utf8: $metadata_json_utf8,
                capability_count: $capability_count,
                capability_get: $capability_get,
                decoder_ext_score_count: None,
                decoder_ext_score_get: None,
                begin_create_decoder_instance: $crate::__st_opt_create_cb!($($begin_create_decoder_instance)?),
                begin_create_dsp_instance: $crate::__st_opt_create_cb!($($begin_create_dsp_instance)?),
                begin_create_source_catalog_instance: $crate::__st_opt_create_cb!($($begin_create_source_catalog_instance)?),
                begin_create_lyrics_provider_instance: $crate::__st_opt_create_cb!($($begin_create_lyrics_provider_instance)?),
                begin_create_output_sink_instance: $crate::__st_opt_create_cb!($($begin_create_output_sink_instance)?),
                begin_quiesce: $crate::__st_opt_create_cb!($($begin_quiesce)?),
                begin_shutdown: $crate::__st_opt_create_cb!($($begin_shutdown)?),
            };

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn stellatune_plugin_entry(
            host: *const stellatune_plugin_api::StHostVTable,
        ) -> *const stellatune_plugin_api::StPluginModule {
            $crate::ffi_guard::guard_with_default("stellatune_plugin_entry", core::ptr::null(), || {
                unsafe { $crate::export::__set_host_vtable(host) };
                &__ST_PLUGIN_MODULE as *const stellatune_plugin_api::StPluginModule
            })
        }
    };
}
