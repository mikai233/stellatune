use anyhow::{Result, anyhow};
use stellatune_plugin_api::StStr;
use stellatune_plugin_api::v2::{StConfigUpdatePlanV2, StLyricsProviderInstanceRefV2};

use super::common::{
    ConfigUpdatePlan, InstanceRuntimeCtx, plan_from_ffi, status_to_result, ststr_from_str,
    take_plugin_string,
};

pub struct LyricsProviderInstanceV2 {
    ctx: InstanceRuntimeCtx,
    handle: *mut core::ffi::c_void,
    vtable: *const stellatune_plugin_api::v2::StLyricsProviderInstanceVTableV2,
}

unsafe impl Send for LyricsProviderInstanceV2 {}

impl LyricsProviderInstanceV2 {
    pub fn from_ffi(ctx: InstanceRuntimeCtx, raw: StLyricsProviderInstanceRefV2) -> Result<Self> {
        if raw.handle.is_null() || raw.vtable.is_null() {
            return Err(anyhow!(
                "lyrics provider instance returned null handle/vtable"
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

    pub fn search_json(&mut self, query_json: &str) -> Result<String> {
        let _call = self.ctx.begin_call();
        let mut out = StStr::empty();
        let status = unsafe {
            ((*self.vtable).search_json_utf8)(self.handle, ststr_from_str(query_json), &mut out)
        };
        status_to_result("Lyrics search_json", status, self.ctx.plugin_free)?;
        Ok(take_plugin_string(out, self.ctx.plugin_free))
    }

    pub fn fetch_json(&mut self, track_json: &str) -> Result<String> {
        let _call = self.ctx.begin_call();
        let mut out = StStr::empty();
        let status = unsafe {
            ((*self.vtable).fetch_json_utf8)(self.handle, ststr_from_str(track_json), &mut out)
        };
        status_to_result("Lyrics fetch_json", status, self.ctx.plugin_free)?;
        Ok(take_plugin_string(out, self.ctx.plugin_free))
    }

    pub fn plan_config_update_json(&self, new_config_json: &str) -> Result<ConfigUpdatePlan> {
        let Some(plan_fn) = (unsafe { (*self.vtable).plan_config_update_json_utf8 }) else {
            return Ok(ConfigUpdatePlan {
                mode: stellatune_plugin_api::v2::StConfigUpdateModeV2::Recreate,
                reason: Some("plugin does not implement plan_config_update".to_string()),
            });
        };
        let _call = self.ctx.begin_call();
        let mut out = StConfigUpdatePlanV2 {
            mode: stellatune_plugin_api::v2::StConfigUpdateModeV2::Reject,
            reason_utf8: StStr::empty(),
        };
        let status = (plan_fn)(self.handle, ststr_from_str(new_config_json), &mut out);
        status_to_result(
            "Lyrics plan_config_update_json",
            status,
            self.ctx.plugin_free,
        )?;
        Ok(plan_from_ffi(out, self.ctx.plugin_free))
    }

    pub fn apply_config_update_json(&mut self, new_config_json: &str) -> Result<()> {
        let Some(apply_fn) = (unsafe { (*self.vtable).apply_config_update_json_utf8 }) else {
            return Err(anyhow!("lyrics apply_config_update not supported"));
        };
        let req = self
            .ctx
            .updates
            .enqueue(self.ctx.instance_id, new_config_json.to_string());
        let _call = self.ctx.begin_call();
        let status = (apply_fn)(self.handle, ststr_from_str(&req.config_json));
        self.ctx.updates.complete(self.ctx.instance_id);
        status_to_result(
            "Lyrics apply_config_update_json",
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
        status_to_result("Lyrics export_state_json", status, self.ctx.plugin_free)?;
        let raw = take_plugin_string(out, self.ctx.plugin_free);
        if raw.is_empty() {
            Ok(None)
        } else {
            Ok(Some(raw))
        }
    }

    pub fn import_state_json(&mut self, state_json: &str) -> Result<()> {
        let Some(import_fn) = (unsafe { (*self.vtable).import_state_json_utf8 }) else {
            return Err(anyhow!("lyrics import_state_json not supported"));
        };
        let _call = self.ctx.begin_call();
        let status = (import_fn)(self.handle, ststr_from_str(state_json));
        status_to_result("Lyrics import_state_json", status, self.ctx.plugin_free)
    }
}

impl Drop for LyricsProviderInstanceV2 {
    fn drop(&mut self) {
        if !self.handle.is_null() && !self.vtable.is_null() {
            let _call = self.ctx.begin_call();
            unsafe { ((*self.vtable).destroy)(self.handle) };
            self.handle = core::ptr::null_mut();
        }
        self.ctx.unregister();
    }
}
