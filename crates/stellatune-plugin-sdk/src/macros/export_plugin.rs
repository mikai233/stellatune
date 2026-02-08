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
        ]
        $(, info_json: $info_json:expr)?
        $(, get_interface: $get_interface:path)?
        $(,)?
    ) => {
        const __ST_PLUGIN_ID: &str = $plugin_id;
        const __ST_PLUGIN_NAME: &str = $plugin_name;

        extern "C" fn __st_plugin_id_utf8() -> $crate::StStr {
            $crate::ststr(__ST_PLUGIN_ID)
        }

        extern "C" fn __st_plugin_name_utf8() -> $crate::StStr {
            $crate::ststr(__ST_PLUGIN_NAME)
        }

        fn __st_plugin_metadata_json() -> &'static str {
            static META: std::sync::OnceLock<String> = std::sync::OnceLock::new();
            META.get_or_init(|| {
                $crate::build_plugin_metadata_json_with_info_json(
                    __ST_PLUGIN_ID,
                    __ST_PLUGIN_NAME,
                    $vmaj,
                    $vmin,
                    $vpatch,
                    $crate::__st_opt_info_json!($($info_json)?),
                )
            })
        }

        extern "C" fn __st_plugin_metadata_json_utf8() -> $crate::StStr {
            let s = __st_plugin_metadata_json();
            $crate::StStr {
                ptr: s.as_ptr(),
                len: s.len(),
            }
        }

        $(
            mod $dec_mod {
                use super::*;

                extern "C" fn type_id_utf8() -> $crate::StStr {
                    $crate::ststr(<$dec_ty as $crate::DecoderDescriptor>::TYPE_ID)
                }

                extern "C" fn probe(path_ext_utf8: $crate::StStr, header: $crate::StSlice<u8>) -> u8 {
                    let ext = unsafe { $crate::ststr_to_str(&path_ext_utf8) }.unwrap_or("");
                    let bytes = if header.ptr.is_null() || header.len == 0 {
                        &[][..]
                    } else {
                        unsafe { core::slice::from_raw_parts(header.ptr, header.len) }
                    };
                    <$dec_ty as $crate::DecoderDescriptor>::probe(ext, bytes)
                }

                extern "C" fn open(
                    args: $crate::StDecoderOpenArgsV1,
                    out: *mut *mut core::ffi::c_void,
                ) -> $crate::StStatus {
                    if out.is_null() || args.io_vtable.is_null() || args.io_handle.is_null() {
                        return $crate::status_err_msg(
                            $crate::ST_ERR_INVALID_ARG,
                            "invalid open args",
                        );
                    }

                    let path = match unsafe { $crate::ststr_to_str(&args.path_utf8) } {
                        Ok(s) => s,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, &e),
                    };
                    let ext = match unsafe { $crate::ststr_to_str(&args.ext_utf8) } {
                        Ok(s) => s,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, &e),
                    };
                    let io = unsafe { $crate::HostIo::from_raw(args.io_vtable, args.io_handle) };

                    match <$dec_ty as $crate::DecoderDescriptor>::open($crate::DecoderOpenArgs {
                        path,
                        ext,
                        io,
                    }) {
                        Ok(dec) => {
                            let info = <$dec_ty as $crate::Decoder>::info(&dec);
                            let boxed = Box::new($crate::DecoderBox {
                                inner: dec,
                                channels: info.spec.channels.max(1),
                            });
                            unsafe { *out = Box::into_raw(boxed) as *mut core::ffi::c_void; }
                            $crate::status_ok()
                        }
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_DECODE, &e),
                    }
                }

                extern "C" fn get_info(
                    handle: *mut core::ffi::c_void,
                    out_info: *mut $crate::StDecoderInfoV1,
                ) -> $crate::StStatus {
                    if handle.is_null() || out_info.is_null() {
                        return $crate::status_err_msg(
                            $crate::ST_ERR_INVALID_ARG,
                            "null handle",
                        );
                    }
                    let boxed = unsafe { &mut *(handle as *mut $crate::DecoderBox<$dec_ty>) };
                    let info = <$dec_ty as $crate::Decoder>::info(&boxed.inner).to_ffi();
                    unsafe { *out_info = info; }
                    $crate::status_ok()
                }

                extern "C" fn get_metadata_json_utf8(
                    handle: *mut core::ffi::c_void,
                    out_json: *mut $crate::StStr,
                ) -> $crate::StStatus {
                    if handle.is_null() || out_json.is_null() {
                        return $crate::status_err_msg(
                            $crate::ST_ERR_INVALID_ARG,
                            "null handle",
                        );
                    }
                    let boxed = unsafe { &mut *(handle as *mut $crate::DecoderBox<$dec_ty>) };
                    match <$dec_ty as $crate::Decoder>::metadata_json(&boxed.inner) {
                        None => {
                            unsafe { *out_json = $crate::StStr::empty(); }
                            $crate::status_ok()
                        }
                        Some(s) => {
                            unsafe { *out_json = $crate::alloc_utf8_bytes(&s); }
                            $crate::status_ok()
                        }
                    }
                }

                extern "C" fn read_interleaved_f32(
                    handle: *mut core::ffi::c_void,
                    frames: u32,
                    out_interleaved: *mut f32,
                    out_frames_read: *mut u32,
                    out_eof: *mut bool,
                ) -> $crate::StStatus {
                    if handle.is_null()
                        || out_interleaved.is_null()
                        || out_frames_read.is_null()
                        || out_eof.is_null()
                    {
                        return $crate::status_err(-1);
                    }

                    let boxed = unsafe { &mut *(handle as *mut $crate::DecoderBox<$dec_ty>) };
                    let len = (frames as usize).saturating_mul(boxed.channels as usize);
                    let out = unsafe { core::slice::from_raw_parts_mut(out_interleaved, len) };

                    match <$dec_ty as $crate::Decoder>::read_interleaved_f32(
                        &mut boxed.inner,
                        frames,
                        out,
                    ) {
                        Ok((n, eof)) => {
                            unsafe {
                                *out_frames_read = n;
                                *out_eof = eof;
                            }
                            $crate::status_ok()
                        }
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_DECODE, &e),
                    }
                }

                extern "C" fn seek_ms(handle: *mut core::ffi::c_void, position_ms: u64) -> $crate::StStatus {
                    if handle.is_null() {
                        return $crate::status_err_msg(
                            $crate::ST_ERR_INVALID_ARG,
                            "null handle",
                        );
                    }
                    let boxed = unsafe { &mut *(handle as *mut $crate::DecoderBox<$dec_ty>) };
                    match <$dec_ty as $crate::Decoder>::seek_ms(&mut boxed.inner, position_ms) {
                        Ok(()) => $crate::status_ok(),
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_UNSUPPORTED, &e),
                    }
                }

                extern "C" fn close(handle: *mut core::ffi::c_void) {
                    if handle.is_null() { return; }
                    unsafe { drop(Box::from_raw(handle as *mut $crate::DecoderBox<$dec_ty>)) };
                }

                pub static VTABLE: $crate::StDecoderVTableV1 = $crate::StDecoderVTableV1 {
                    type_id_utf8,
                    probe,
                    open,
                    get_info,
                    get_metadata_json_utf8: Some(get_metadata_json_utf8),
                    read_interleaved_f32,
                    seek_ms: if <$dec_ty as $crate::DecoderDescriptor>::SUPPORTS_SEEK {
                        Some(seek_ms)
                    } else {
                        None
                    },
                    close,
                };
            }
        )*

        const __ST_DEC_COUNT: usize = 0 $(+ { let _ = core::mem::size_of::<$dec_ty>(); 1 })*;
        extern "C" fn __st_decoder_count() -> usize { __ST_DEC_COUNT }
        extern "C" fn __st_decoder_get(index: usize) -> *const $crate::StDecoderVTableV1 {
            let mut i = 0usize;
            $(
                if index == i {
                    return &$dec_mod::VTABLE;
                }
                i += 1;
            )*
            core::ptr::null()
        }

        $(
            mod $dsp_mod {
                use super::*;

                extern "C" fn type_id_utf8() -> $crate::StStr {
                    $crate::ststr(<$dsp_ty as $crate::DspDescriptor>::TYPE_ID)
                }
                extern "C" fn display_name_utf8() -> $crate::StStr {
                    $crate::ststr(<$dsp_ty as $crate::DspDescriptor>::DISPLAY_NAME)
                }
                extern "C" fn config_schema_json_utf8() -> $crate::StStr {
                    $crate::ststr(<$dsp_ty as $crate::DspDescriptor>::CONFIG_SCHEMA_JSON)
                }
                extern "C" fn default_config_json_utf8() -> $crate::StStr {
                    $crate::ststr(<$dsp_ty as $crate::DspDescriptor>::DEFAULT_CONFIG_JSON)
                }

                extern "C" fn create(
                    sample_rate: u32,
                    channels: u16,
                    config_json_utf8: $crate::StStr,
                    out: *mut *mut core::ffi::c_void,
                ) -> $crate::StStatus {
                    if out.is_null() {
                        return $crate::status_err(-1);
                    }
                    let json = match unsafe { $crate::ststr_to_str(&config_json_utf8) } {
                        Ok(s) => s,
                        Err(_) => return $crate::status_err(-2),
                    };
                    let spec = $crate::StAudioSpec {
                        sample_rate,
                        channels,
                        reserved: 0,
                    };
                    let channels = channels.max(1);

                    match <$dsp_ty as $crate::DspDescriptor>::create(spec, json) {
                        Ok(dsp) => {
                            let boxed = Box::new($crate::DspBox {
                                inner: dsp,
                                channels,
                            });
                            unsafe { *out = Box::into_raw(boxed) as *mut core::ffi::c_void; }
                            $crate::status_ok()
                        }
                        Err(_) => $crate::status_err(-3),
                    }
                }

                extern "C" fn set_config_json_utf8(
                    handle: *mut core::ffi::c_void,
                    config_json_utf8: $crate::StStr,
                ) -> $crate::StStatus {
                    if handle.is_null() {
                        return $crate::status_err(-1);
                    }
                    let json = match unsafe { $crate::ststr_to_str(&config_json_utf8) } {
                        Ok(s) => s,
                        Err(_) => return $crate::status_err(-2),
                    };
                    let boxed = unsafe { &mut *(handle as *mut $crate::DspBox<$dsp_ty>) };
                    match <$dsp_ty as $crate::Dsp>::set_config_json(&mut boxed.inner, json) {
                        Ok(()) => $crate::status_ok(),
                        Err(_) => $crate::status_err(-3),
                    }
                }

                extern "C" fn process_interleaved_f32_in_place(
                    handle: *mut core::ffi::c_void,
                    samples: *mut f32,
                    frames: u32,
                ) {
                    if handle.is_null() || samples.is_null() {
                        return;
                    }
                    let boxed = unsafe { &mut *(handle as *mut $crate::DspBox<$dsp_ty>) };
                    let len = (frames as usize).saturating_mul(boxed.channels as usize);
                    let buf = unsafe { core::slice::from_raw_parts_mut(samples, len) };
                    <$dsp_ty as $crate::Dsp>::process_interleaved_f32_in_place(
                        &mut boxed.inner,
                        buf,
                        frames,
                    );
                }

                extern "C" fn drop_handle(handle: *mut core::ffi::c_void) {
                    if handle.is_null() { return; }
                    unsafe { drop(Box::from_raw(handle as *mut $crate::DspBox<$dsp_ty>)) };
                }

                extern "C" fn supported_layouts() -> u32 {
                    <$dsp_ty as $crate::DspDescriptor>::SUPPORTED_LAYOUTS
                }

                extern "C" fn output_channels() -> u16 {
                    <$dsp_ty as $crate::DspDescriptor>::OUTPUT_CHANNELS
                }

                pub static VTABLE: $crate::StDspVTableV1 = $crate::StDspVTableV1 {
                    type_id_utf8,
                    display_name_utf8,
                    config_schema_json_utf8,
                    default_config_json_utf8,
                    create,
                    set_config_json_utf8,
                    process_interleaved_f32_in_place,
                    drop: drop_handle,
                    supported_layouts,
                    output_channels,
                };
            }
        )*

        const __ST_DSP_COUNT: usize = 0 $(+ { let _ = core::mem::size_of::<$dsp_ty>(); 1 })*;
        extern "C" fn __st_dsp_count() -> usize { __ST_DSP_COUNT }

        extern "C" fn __st_dsp_get(index: usize) -> *const $crate::StDspVTableV1 {
            let mut i = 0usize;
            $(
                if index == i {
                    return &$dsp_mod::VTABLE;
                }
                i += 1;
            )*
            core::ptr::null()
        }

        static __ST_PLUGIN_VTABLE: $crate::StPluginVTableV1 = $crate::StPluginVTableV1 {
            api_version: $crate::STELLATUNE_PLUGIN_API_VERSION_V1,
            plugin_version: $crate::StVersion { major: $vmaj, minor: $vmin, patch: $vpatch, reserved: 0 },
            plugin_free: Some($crate::plugin_free),
            id_utf8: __st_plugin_id_utf8,
            name_utf8: __st_plugin_name_utf8,
            metadata_json_utf8: __st_plugin_metadata_json_utf8,
            decoder_count: __st_decoder_count,
            decoder_get: __st_decoder_get,
            dsp_count: __st_dsp_count,
            dsp_get: __st_dsp_get,
            get_interface: $crate::__st_opt_get_interface!($($get_interface)?),
        };

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn stellatune_plugin_entry_v1(
            host: *const $crate::StHostVTableV1,
        ) -> *const $crate::StPluginVTableV1 {
            unsafe { $crate::__set_host_vtable_v1(host) };
            &__ST_PLUGIN_VTABLE
        }
    };
}
