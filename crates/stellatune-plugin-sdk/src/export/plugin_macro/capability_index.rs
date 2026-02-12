#[doc(hidden)]
#[macro_export]
macro_rules! __st_export_capability_index {
    (
        decoders: [$( $dec_mod:ident => $dec_ty:ty ),* $(,)?],
        dsps: [$( $dsp_mod:ident => $dsp_ty:ty ),* $(,)?],
        source_catalogs: [$( $source_mod:ident => $source_ty:ty ),* $(,)?],
        lyrics_providers: [$( $lyrics_mod:ident => $lyrics_ty:ty ),* $(,)?],
        output_sinks: [$( $sink_mod:ident => $sink_ty:ty ),* $(,)?],
    ) => {
        const __ST_DECODER_COUNT: usize = 0 $(+ { let _ = core::mem::size_of::<$dec_ty>(); 1 })*;
        const __ST_DSP_COUNT: usize = 0 $(+ { let _ = core::mem::size_of::<$dsp_ty>(); 1 })*;
        const __ST_SOURCE_COUNT: usize = 0 $(+ { let _ = core::mem::size_of::<$source_ty>(); 1 })*;
        const __ST_LYRICS_COUNT: usize = 0 $(+ { let _ = core::mem::size_of::<$lyrics_ty>(); 1 })*;
        const __ST_SINK_COUNT: usize = 0 $(+ { let _ = core::mem::size_of::<$sink_ty>(); 1 })*;
        const __ST_CAPABILITY_COUNT: usize = __ST_DECODER_COUNT
            + __ST_DSP_COUNT
            + __ST_SOURCE_COUNT
            + __ST_LYRICS_COUNT
            + __ST_SINK_COUNT;

        extern "C" fn __st_capability_count() -> usize {
            $crate::ffi_guard::guard_with_default("__st_capability_count", 0, || {
                __ST_CAPABILITY_COUNT
            })
        }

        extern "C" fn __st_capability_get(
            index: usize,
        ) -> *const stellatune_plugin_api::StCapabilityDescriptor {
            $crate::ffi_guard::guard_with_default("__st_capability_get", core::ptr::null(), || {
                let mut i = 0usize;
                $(
                    if index == i {
                        return &$dec_mod::CAP_DESC as *const _;
                    }
                    i += 1;
                )*
                $(
                    if index == i {
                        return &$dsp_mod::CAP_DESC as *const _;
                    }
                    i += 1;
                )*
                $(
                    if index == i {
                        return &$source_mod::CAP_DESC as *const _;
                    }
                    i += 1;
                )*
                $(
                    if index == i {
                        return &$lyrics_mod::CAP_DESC as *const _;
                    }
                    i += 1;
                )*
                $(
                    if index == i {
                        return &$sink_mod::CAP_DESC as *const _;
                    }
                    i += 1;
                )*
                core::ptr::null()
            })
        }
    };
}
