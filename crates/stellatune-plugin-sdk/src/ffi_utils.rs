use core::ffi::c_void;

use crate::{StStatus, StStr};

#[inline]
pub const fn ststr(s: &'static str) -> StStr {
    StStr {
        ptr: s.as_ptr(),
        len: s.len(),
    }
}

#[inline]
pub fn status_ok() -> StStatus {
    StStatus::ok()
}

#[inline]
pub fn status_err(code: i32) -> StStatus {
    StStatus {
        code,
        message: StStr::empty(),
    }
}

pub extern "C" fn plugin_free(ptr: *mut c_void, len: usize, align: usize) {
    if ptr.is_null() || len == 0 {
        return;
    }
    let align = align.max(1);
    // Safety: allocated by `alloc_utf8_bytes` with the same layout.
    unsafe {
        let layout = std::alloc::Layout::from_size_align_unchecked(len, align);
        std::alloc::dealloc(ptr as *mut u8, layout);
    }
}

pub fn alloc_utf8_bytes(s: &str) -> StStr {
    if s.is_empty() {
        return StStr::empty();
    }
    let bytes = s.as_bytes();
    let len = bytes.len();
    let layout = std::alloc::Layout::from_size_align(len, 1).expect("layout");
    // Safety: layout is valid, and we copy exactly `len` bytes.
    unsafe {
        let ptr = std::alloc::alloc(layout);
        if ptr.is_null() {
            return StStr::empty();
        }
        core::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, len);
        StStr { ptr, len }
    }
}

pub fn status_err_msg(code: i32, msg: &str) -> StStatus {
    StStatus {
        code,
        message: alloc_utf8_bytes(msg),
    }
}

/// # Safety
///
/// The caller must ensure that the `StStr` contains a valid pointer to a memory region
/// of at least `s.len` bytes.
pub unsafe fn ststr_to_str(s: &StStr) -> Result<&str, String> {
    if s.ptr.is_null() || s.len == 0 {
        return Ok("");
    }
    let bytes = unsafe { core::slice::from_raw_parts(s.ptr, s.len) };
    core::str::from_utf8(bytes).map_err(|_| "invalid utf-8".to_string())
}
