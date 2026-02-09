use anyhow::{Result, anyhow};
use stellatune_plugin_api::{StConfigUpdatePlan, StSourceCatalogInstanceRef};
use stellatune_plugin_api::{StIoVTable, StStr};

use super::common::{
    ConfigUpdatePlan, InstanceRuntimeCtx, plan_from_ffi, status_to_result, ststr_from_str,
    take_plugin_string,
};

pub struct SourceCatalogInstance {
    ctx: InstanceRuntimeCtx,
    handle: *mut core::ffi::c_void,
    vtable: *const stellatune_plugin_api::StSourceCatalogInstanceVTable,
}

unsafe impl Send for SourceCatalogInstance {}

#[derive(Debug, Clone, Copy)]
pub struct SourceOpenStreamResult {
    pub io_vtable: *const StIoVTable,
    pub io_handle: *mut core::ffi::c_void,
}

impl SourceCatalogInstance {
    pub fn from_ffi(ctx: InstanceRuntimeCtx, raw: StSourceCatalogInstanceRef) -> Result<Self> {
        if raw.handle.is_null() || raw.vtable.is_null() {
            return Err(anyhow!(
                "source catalog instance returned null handle/vtable"
            ));
        }
        Ok(Self {
            ctx,
            handle: raw.handle,
            vtable: raw.vtable,
        })
    }

    pub fn instance_id(&self) -> crate::runtime::InstanceId {
        self.ctx.instance_id
    }

    pub fn list_items_json(&mut self, request_json: &str) -> Result<String> {
        let _call = self.ctx.begin_call();
        let mut out = StStr::empty();
        let status = unsafe {
            ((*self.vtable).list_items_json_utf8)(
                self.handle,
                ststr_from_str(request_json),
                &mut out,
            )
        };
        status_to_result("Source list_items_json", status, self.ctx.plugin_free)?;
        Ok(take_plugin_string(out, self.ctx.plugin_free))
    }

    pub fn open_stream(
        &mut self,
        track_json: &str,
    ) -> Result<(SourceOpenStreamResult, Option<String>)> {
        let _call = self.ctx.begin_call();
        let mut out_io_vtable: *const StIoVTable = core::ptr::null();
        let mut out_io_handle: *mut core::ffi::c_void = core::ptr::null_mut();
        let mut out_meta = StStr::empty();
        let status = unsafe {
            ((*self.vtable).open_stream)(
                self.handle,
                ststr_from_str(track_json),
                &mut out_io_vtable,
                &mut out_io_handle,
                &mut out_meta,
            )
        };
        status_to_result("Source open_stream", status, self.ctx.plugin_free)?;
        if out_io_vtable.is_null() || out_io_handle.is_null() {
            return Err(anyhow!(
                "source open_stream returned null io_vtable/io_handle"
            ));
        }
        let meta = take_plugin_string(out_meta, self.ctx.plugin_free);
        Ok((
            SourceOpenStreamResult {
                io_vtable: out_io_vtable,
                io_handle: out_io_handle,
            },
            if meta.is_empty() { None } else { Some(meta) },
        ))
    }

    pub fn close_stream(&mut self, io_handle: *mut core::ffi::c_void) {
        if io_handle.is_null() {
            return;
        }
        let _call = self.ctx.begin_call();
        unsafe { ((*self.vtable).close_stream)(self.handle, io_handle) };
    }

    pub fn plan_config_update_json(&self, new_config_json: &str) -> Result<ConfigUpdatePlan> {
        let Some(plan_fn) = (unsafe { (*self.vtable).plan_config_update_json_utf8 }) else {
            return Ok(ConfigUpdatePlan {
                mode: stellatune_plugin_api::StConfigUpdateMode::Recreate,
                reason: Some("plugin does not implement plan_config_update".to_string()),
            });
        };
        let _call = self.ctx.begin_call();
        let mut out = StConfigUpdatePlan {
            mode: stellatune_plugin_api::StConfigUpdateMode::Reject,
            reason_utf8: StStr::empty(),
        };
        let status = (plan_fn)(self.handle, ststr_from_str(new_config_json), &mut out);
        status_to_result(
            "Source plan_config_update_json",
            status,
            self.ctx.plugin_free,
        )?;
        Ok(plan_from_ffi(out, self.ctx.plugin_free))
    }

    pub fn apply_config_update_json(&mut self, new_config_json: &str) -> Result<()> {
        let Some(apply_fn) = (unsafe { (*self.vtable).apply_config_update_json_utf8 }) else {
            return Err(anyhow!("source apply_config_update not supported"));
        };
        let req = self
            .ctx
            .updates
            .enqueue(self.ctx.instance_id, new_config_json.to_string());
        let _call = self.ctx.begin_call();
        let status = (apply_fn)(self.handle, ststr_from_str(&req.config_json));
        self.ctx.updates.complete(self.ctx.instance_id);
        status_to_result(
            "Source apply_config_update_json",
            status,
            self.ctx.plugin_free,
        )
    }

    pub fn export_state_json(&self) -> Result<Option<String>> {
        let Some(export_fn) = (unsafe { (*self.vtable).export_state_json_utf8 }) else {
            return Ok(None);
        };
        let _call = self.ctx.begin_call();
        let mut out = StStr::empty();
        let status = (export_fn)(self.handle, &mut out);
        status_to_result("Source export_state_json", status, self.ctx.plugin_free)?;
        let raw = take_plugin_string(out, self.ctx.plugin_free);
        if raw.is_empty() {
            Ok(None)
        } else {
            Ok(Some(raw))
        }
    }

    pub fn import_state_json(&mut self, state_json: &str) -> Result<()> {
        let Some(import_fn) = (unsafe { (*self.vtable).import_state_json_utf8 }) else {
            return Err(anyhow!("source import_state_json not supported"));
        };
        let _call = self.ctx.begin_call();
        let status = (import_fn)(self.handle, ststr_from_str(state_json));
        status_to_result("Source import_state_json", status, self.ctx.plugin_free)
    }
}

impl Drop for SourceCatalogInstance {
    fn drop(&mut self) {
        if !self.handle.is_null() && !self.vtable.is_null() {
            let _call = self.ctx.begin_call();
            unsafe { ((*self.vtable).destroy)(self.handle) };
            self.handle = core::ptr::null_mut();
        }
        self.ctx.unregister();
    }
}
