#[doc(hidden)]
#[macro_export]
macro_rules! __st_export_module_entry {
    ($vmaj:literal, $vmin:literal, $vpatch:literal) => {
        static __ST_PLUGIN_MODULE_FULL: stellatune_plugin_api::StPluginModule =
            stellatune_plugin_api::StPluginModule {
                api_version: stellatune_plugin_api::STELLATUNE_PLUGIN_API_VERSION,
                plugin_version: $crate::StVersion {
                    major: $vmaj,
                    minor: $vmin,
                    patch: $vpatch,
                    reserved: 0,
                },
                plugin_free: Some($crate::plugin_free),
                metadata_json_utf8: __st_plugin_metadata_json_utf8,
                capability_count: __st_capability_count,
                capability_get: __st_capability_get,
                decoder_ext_score_count: Some(__st_decoder_ext_score_count),
                decoder_ext_score_get: Some(__st_decoder_ext_score_get),
                create_decoder_instance: Some(__st_create_decoder_instance),
                create_dsp_instance: Some(__st_create_dsp_instance),
                create_source_catalog_instance: Some(__st_create_source_catalog_instance),
                create_lyrics_provider_instance: Some(__st_create_lyrics_provider_instance),
                create_output_sink_instance: Some(__st_create_output_sink_instance),
                shutdown: None,
            };

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn stellatune_plugin_entry(
            host: *const stellatune_plugin_api::StHostVTable,
        ) -> *const stellatune_plugin_api::StPluginModule {
            $crate::ffi_guard::guard_with_default(
                "stellatune_plugin_entry",
                core::ptr::null(),
                || {
                    unsafe { $crate::export::__set_host_vtable(host) };
                    &__ST_PLUGIN_MODULE_FULL as *const stellatune_plugin_api::StPluginModule
                },
            )
        }
    };
}
