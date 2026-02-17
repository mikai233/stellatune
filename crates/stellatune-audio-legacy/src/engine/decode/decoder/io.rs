use std::ffi::c_void;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::slice;

use stellatune_plugin_api::{
    ST_ERR_INVALID_ARG, ST_ERR_IO, StIoVTable, StSeekWhence, StStatus, StStr,
};

use crate::engine::control::source_close_stream_via_runtime_blocking;

pub(crate) struct LocalFileIoHandle {
    file: File,
}

pub(crate) enum DecoderIoOwner {
    Local(Box<LocalFileIoHandle>),
    Source {
        stream_id: u64,
        lease_id: u64,
        io_handle_addr: usize,
    },
}

impl DecoderIoOwner {
    pub(super) fn io_vtable_ptr(&self) -> *const StIoVTable {
        match self {
            Self::Local(_) => &LOCAL_FILE_IO_VTABLE as *const StIoVTable,
            Self::Source { .. } => core::ptr::null(),
        }
    }

    pub(super) fn io_handle_ptr(&mut self) -> *mut c_void {
        match self {
            Self::Local(file) => (&mut **file) as *mut LocalFileIoHandle as *mut c_void,
            Self::Source { io_handle_addr, .. } => *io_handle_addr as *mut c_void,
        }
    }

    pub(super) fn local(path: &str) -> Result<Self, String> {
        let file =
            File::open(path).map_err(|e| format!("failed to open local file `{path}`: {e}"))?;
        Ok(Self::Local(Box::new(LocalFileIoHandle { file })))
    }
}

impl Drop for DecoderIoOwner {
    fn drop(&mut self) {
        if let Self::Source {
            stream_id,
            lease_id,
            io_handle_addr,
        } = self
            && *io_handle_addr != 0
        {
            let _ = source_close_stream_via_runtime_blocking(*stream_id);
            *io_handle_addr = 0;
            *stream_id = 0;
            *lease_id = 0;
        }
    }
}

fn status_code(code: i32) -> StStatus {
    StStatus {
        code,
        message: StStr::empty(),
    }
}

extern "C" fn local_io_read(
    handle: *mut c_void,
    out: *mut u8,
    len: usize,
    out_read: *mut usize,
) -> StStatus {
    if handle.is_null() || out_read.is_null() || (len > 0 && out.is_null()) {
        return status_code(ST_ERR_INVALID_ARG);
    }
    let state = unsafe { &mut *(handle as *mut LocalFileIoHandle) };
    let out_slice: &mut [u8] = if len == 0 {
        &mut []
    } else {
        unsafe { slice::from_raw_parts_mut(out, len) }
    };
    match state.file.read(out_slice) {
        Ok(n) => {
            unsafe {
                *out_read = n;
            }
            StStatus::ok()
        },
        Err(_) => status_code(ST_ERR_IO),
    }
}

extern "C" fn local_io_seek(
    handle: *mut c_void,
    offset: i64,
    whence: StSeekWhence,
    out_pos: *mut u64,
) -> StStatus {
    if handle.is_null() || out_pos.is_null() {
        return status_code(ST_ERR_INVALID_ARG);
    }
    let state = unsafe { &mut *(handle as *mut LocalFileIoHandle) };
    let seek_from = match whence {
        StSeekWhence::Start => {
            if offset < 0 {
                return status_code(ST_ERR_INVALID_ARG);
            }
            SeekFrom::Start(offset as u64)
        },
        StSeekWhence::Current => SeekFrom::Current(offset),
        StSeekWhence::End => SeekFrom::End(offset),
    };
    match state.file.seek(seek_from) {
        Ok(pos) => {
            unsafe {
                *out_pos = pos;
            }
            StStatus::ok()
        },
        Err(_) => status_code(ST_ERR_IO),
    }
}

extern "C" fn local_io_tell(handle: *mut c_void, out_pos: *mut u64) -> StStatus {
    if handle.is_null() || out_pos.is_null() {
        return status_code(ST_ERR_INVALID_ARG);
    }
    let state = unsafe { &mut *(handle as *mut LocalFileIoHandle) };
    match state.file.stream_position() {
        Ok(pos) => {
            unsafe {
                *out_pos = pos;
            }
            StStatus::ok()
        },
        Err(_) => status_code(ST_ERR_IO),
    }
}

extern "C" fn local_io_size(handle: *mut c_void, out_size: *mut u64) -> StStatus {
    if handle.is_null() || out_size.is_null() {
        return status_code(ST_ERR_INVALID_ARG);
    }
    let state = unsafe { &mut *(handle as *mut LocalFileIoHandle) };
    match state.file.metadata() {
        Ok(meta) => {
            unsafe {
                *out_size = meta.len();
            }
            StStatus::ok()
        },
        Err(_) => status_code(ST_ERR_IO),
    }
}

static LOCAL_FILE_IO_VTABLE: StIoVTable = StIoVTable {
    read: local_io_read,
    seek: Some(local_io_seek),
    tell: Some(local_io_tell),
    size: Some(local_io_size),
};
