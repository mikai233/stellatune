mod capability_index;
mod decoder_dispatch;
mod decoder_modules;
mod dsp_modules;
mod instance_factories;
mod lyrics_modules;
mod module_entry;
mod output_modules;
mod source_modules;

#[macro_export]
macro_rules! export_plugin {
    (
        id: $plugin_id:literal,
        name: $plugin_name:literal,
        version: ($vmaj:literal, $vmin:literal, $vpatch:literal),
        decoders: [
            $($dec_mod:ident => $dec_ty:ty),* $(,)?
        ],
        dsps: [
            $($dsp_mod:ident => $dsp_ty:ty),* $(,)?
        ],
        source_catalogs: [
            $($source_mod:ident => $source_ty:ty),* $(,)?
        ],
        lyrics_providers: [
            $($lyrics_mod:ident => $lyrics_ty:ty),* $(,)?
        ],
        output_sinks: [
            $($sink_mod:ident => $sink_ty:ty),* $(,)?
        ]
        $(, info: $info:expr)?
        $(,)?
    ) => {
        const __ST_PLUGIN_ID: &str = $plugin_id;
        const __ST_PLUGIN_NAME: &str = $plugin_name;

        fn __st_plugin_metadata_json() -> &'static str {
            static META: std::sync::OnceLock<String> = std::sync::OnceLock::new();
            META.get_or_init(|| {
                let metadata = $crate::build_plugin_metadata_with_info(
                    __ST_PLUGIN_ID,
                    __ST_PLUGIN_NAME,
                    $vmaj,
                    $vmin,
                    $vpatch,
                    $crate::__st_opt_info!($($info)?),
                );
                $crate::__private::serde_json::to_string(&metadata)
                    .expect("export_plugin! metadata must be serializable")
            })
        }

        extern "C" fn __st_plugin_metadata_json_utf8() -> $crate::StStr {
            let s = __st_plugin_metadata_json();
            $crate::StStr {
                ptr: s.as_ptr(),
                len: s.len(),
            }
        }

        $crate::__st_export_decoder_modules!($( $dec_mod => $dec_ty ),*);
        $crate::__st_export_dsp_modules!($( $dsp_mod => $dsp_ty ),*);
        $crate::__st_export_source_modules!($( $source_mod => $source_ty ),*);
        $crate::__st_export_lyrics_modules!($( $lyrics_mod => $lyrics_ty ),*);
        $crate::__st_export_output_modules!($( $sink_mod => $sink_ty ),*);
        $crate::__st_export_capability_index!(
            decoders: [$( $dec_mod => $dec_ty ),*],
            dsps: [$( $dsp_mod => $dsp_ty ),*],
            source_catalogs: [$( $source_mod => $source_ty ),*],
            lyrics_providers: [$( $lyrics_mod => $lyrics_ty ),*],
            output_sinks: [$( $sink_mod => $sink_ty ),*],
        );
        $crate::__st_export_decoder_dispatch!($( $dec_mod => $dec_ty ),*);
        $crate::__st_export_instance_factories!(
            dsps: [$( $dsp_mod => $dsp_ty ),*],
            source_catalogs: [$( $source_mod => $source_ty ),*],
            lyrics_providers: [$( $lyrics_mod => $lyrics_ty ),*],
            output_sinks: [$( $sink_mod => $sink_ty ),*],
        );
        $crate::__st_export_module_entry!($vmaj, $vmin, $vpatch);
    };
}
