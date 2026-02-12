#[doc(hidden)]
#[macro_export]
macro_rules! __st_export_instance_factories {
    (
        dsps: [$( $dsp_mod:ident => $dsp_ty:ty ),* $(,)?],
        source_catalogs: [$( $source_mod:ident => $source_ty:ty ),* $(,)?],
        lyrics_providers: [$( $lyrics_mod:ident => $lyrics_ty:ty ),* $(,)?],
        output_sinks: [$( $sink_mod:ident => $sink_ty:ty ),* $(,)?],
    ) => {
        extern "C" fn __st_create_dsp_instance(
            type_id_utf8: $crate::StStr,
            sample_rate: u32,
            channels: u16,
            config_json_utf8: $crate::StStr,
            out_instance: *mut stellatune_plugin_api::StDspInstanceRef,
        ) -> $crate::StStatus {
            $crate::ffi_guard::guard_status("__st_create_dsp_instance", || {
                let type_id = match unsafe { $crate::ststr_to_str(&type_id_utf8) } {
                    Ok(s) => s,
                    Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
                };
                $(
                    if type_id == <$dsp_ty as $crate::instance::DspDescriptor>::TYPE_ID {
                        return $dsp_mod::create_instance(
                            sample_rate,
                            channels,
                            config_json_utf8,
                            out_instance,
                        );
                    }
                )*
                $crate::status_err_msg($crate::ST_ERR_UNSUPPORTED, "dsp type unsupported")
            })
        }

        extern "C" fn __st_create_source_catalog_instance(
            type_id_utf8: $crate::StStr,
            config_json_utf8: $crate::StStr,
            out_instance: *mut stellatune_plugin_api::StSourceCatalogInstanceRef,
        ) -> $crate::StStatus {
            $crate::ffi_guard::guard_status("__st_create_source_catalog_instance", || {
                let type_id = match unsafe { $crate::ststr_to_str(&type_id_utf8) } {
                    Ok(s) => s,
                    Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
                };
                $(
                    if type_id == <$source_ty as $crate::instance::SourceCatalogDescriptor>::TYPE_ID {
                        return $source_mod::create_instance(config_json_utf8, out_instance);
                    }
                )*
                $crate::status_err_msg($crate::ST_ERR_UNSUPPORTED, "source catalog type unsupported")
            })
        }

        extern "C" fn __st_create_lyrics_provider_instance(
            type_id_utf8: $crate::StStr,
            config_json_utf8: $crate::StStr,
            out_instance: *mut stellatune_plugin_api::StLyricsProviderInstanceRef,
        ) -> $crate::StStatus {
            $crate::ffi_guard::guard_status("__st_create_lyrics_provider_instance", || {
                let type_id = match unsafe { $crate::ststr_to_str(&type_id_utf8) } {
                    Ok(s) => s,
                    Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
                };
                $(
                    if type_id == <$lyrics_ty as $crate::instance::LyricsProviderDescriptor>::TYPE_ID {
                        return $lyrics_mod::create_instance(config_json_utf8, out_instance);
                    }
                )*
                $crate::status_err_msg($crate::ST_ERR_UNSUPPORTED, "lyrics provider type unsupported")
            })
        }

        extern "C" fn __st_create_output_sink_instance(
            type_id_utf8: $crate::StStr,
            config_json_utf8: $crate::StStr,
            out_instance: *mut stellatune_plugin_api::StOutputSinkInstanceRef,
        ) -> $crate::StStatus {
            $crate::ffi_guard::guard_status("__st_create_output_sink_instance", || {
                let type_id = match unsafe { $crate::ststr_to_str(&type_id_utf8) } {
                    Ok(s) => s,
                    Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
                };
                $(
                    if type_id == <$sink_ty as $crate::instance::OutputSinkDescriptor>::TYPE_ID {
                        return $sink_mod::create_instance(config_json_utf8, out_instance);
                    }
                )*
                $crate::status_err_msg($crate::ST_ERR_UNSUPPORTED, "output sink type unsupported")
            })
        }
    };
}
