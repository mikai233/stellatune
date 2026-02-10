use core::sync::atomic::{AtomicPtr, Ordering};

use stellatune_plugin_api::{
    STELLATUNE_PLUGIN_API_VERSION, STELLATUNE_PLUGIN_ENTRY_SYMBOL, StCapabilityDescriptor,
    StCapabilityKind, StHostVTable, StPluginModule,
};

use crate::{StStr, ststr};

static HOST_VTABLE: AtomicPtr<StHostVTable> = AtomicPtr::new(core::ptr::null_mut());

#[doc(hidden)]
pub unsafe fn __set_host_vtable(host: *const StHostVTable) {
    HOST_VTABLE.store(host as *mut StHostVTable, Ordering::Release);
}

pub fn host_vtable_raw() -> Option<*const StHostVTable> {
    let p = HOST_VTABLE.load(Ordering::Acquire);
    if p.is_null() {
        None
    } else {
        Some(p as *const _)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityDescriptorStatic {
    pub kind: StCapabilityKind,
    pub type_id_utf8: &'static str,
    pub display_name_utf8: &'static str,
    pub config_schema_json_utf8: &'static str,
    pub default_config_json_utf8: &'static str,
}

impl CapabilityDescriptorStatic {
    pub const fn to_ffi(self) -> StCapabilityDescriptor {
        StCapabilityDescriptor {
            kind: self.kind,
            type_id_utf8: StStr {
                ptr: self.type_id_utf8.as_ptr(),
                len: self.type_id_utf8.len(),
            },
            display_name_utf8: StStr {
                ptr: self.display_name_utf8.as_ptr(),
                len: self.display_name_utf8.len(),
            },
            config_schema_json_utf8: StStr {
                ptr: self.config_schema_json_utf8.as_ptr(),
                len: self.config_schema_json_utf8.len(),
            },
            default_config_json_utf8: StStr {
                ptr: self.default_config_json_utf8.as_ptr(),
                len: self.default_config_json_utf8.len(),
            },
            reserved0: 0,
            reserved1: 0,
        }
    }
}

pub fn static_str_ststr(s: &'static str) -> StStr {
    ststr(s)
}

pub fn entry_symbol() -> &'static str {
    STELLATUNE_PLUGIN_ENTRY_SYMBOL
}

pub fn module_api_version() -> u32 {
    STELLATUNE_PLUGIN_API_VERSION
}

pub fn module_is_current(module: &StPluginModule) -> bool {
    module.api_version == STELLATUNE_PLUGIN_API_VERSION
}
