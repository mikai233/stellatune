#[doc(hidden)]
#[macro_export]
macro_rules! __st_export_decoder_dispatch {
    ($( $dec_mod:ident => $dec_ty:ty ),* $(,)?) => {
        extern "C" fn __st_decoder_ext_score_count(type_id_utf8: $crate::StStr) -> usize {
            let type_id = match unsafe { $crate::ststr_to_str(&type_id_utf8) } {
                Ok(s) => s,
                Err(_) => return 0,
            };
            $(
                if type_id == <$dec_ty as $crate::instance::DecoderDescriptor>::TYPE_ID {
                    return $dec_mod::ext_score_count();
                }
            )*
            0
        }

        extern "C" fn __st_decoder_ext_score_get(
            type_id_utf8: $crate::StStr,
            index: usize,
        ) -> *const stellatune_plugin_api::StDecoderExtScore {
            let type_id = match unsafe { $crate::ststr_to_str(&type_id_utf8) } {
                Ok(s) => s,
                Err(_) => return core::ptr::null(),
            };
            $(
                if type_id == <$dec_ty as $crate::instance::DecoderDescriptor>::TYPE_ID {
                    return $dec_mod::ext_score_get(index);
                }
            )*
            core::ptr::null()
        }

        extern "C" fn __st_create_decoder_instance(
            type_id_utf8: $crate::StStr,
            config_json_utf8: $crate::StStr,
            out_instance: *mut stellatune_plugin_api::StDecoderInstanceRef,
        ) -> $crate::StStatus {
            let type_id = match unsafe { $crate::ststr_to_str(&type_id_utf8) } {
                Ok(s) => s,
                Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
            };
            $(
                if type_id == <$dec_ty as $crate::instance::DecoderDescriptor>::TYPE_ID {
                    return $dec_mod::create_instance(config_json_utf8, out_instance);
                }
            )*
            $crate::status_err_msg($crate::ST_ERR_UNSUPPORTED, "decoder type unsupported")
        }
    };
}
