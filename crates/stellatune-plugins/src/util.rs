use stellatune_plugin_api::StStr;

pub unsafe fn ststr_to_string_lossy(s: StStr) -> String {
    if s.ptr.is_null() || s.len == 0 {
        return String::new();
    }
    let bytes = unsafe { core::slice::from_raw_parts(s.ptr, s.len) };
    String::from_utf8_lossy(bytes).into_owned()
}
