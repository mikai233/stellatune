use anyhow::{Result, anyhow};
use stellatune_plugin_api::StStr;
use stellatune_plugin_api::v2::{StConfigUpdatePlanV2, StDspInstanceRefV2};

use super::common::{
    ConfigUpdatePlan, InstanceRuntimeCtx, plan_from_ffi, status_to_result, ststr_from_str,
    take_plugin_string,
};

pub struct DspInstanceV2 {
    ctx: InstanceRuntimeCtx,
    handle: *mut core::ffi::c_void,
    vtable: *const stellatune_plugin_api::v2::StDspInstanceVTableV2,
}

unsafe impl Send for DspInstanceV2 {}

impl DspInstanceV2 {
    pub fn from_ffi(ctx: InstanceRuntimeCtx, raw: StDspInstanceRefV2) -> Result<Self> {
        if raw.handle.is_null() || raw.vtable.is_null() {
            return Err(anyhow!("dsp instance returned null handle/vtable"));
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

    pub fn process_interleaved_f32_in_place(&mut self, samples: &mut [f32], frames: u32) {
        let _call = self.ctx.begin_call();
        unsafe {
            ((*self.vtable).process_interleaved_f32_in_place)(
                self.handle,
                samples.as_mut_ptr(),
                frames,
            )
        };
    }

    pub fn supported_layouts(&self) -> u32 {
        let _call = self.ctx.begin_call();
        unsafe { ((*self.vtable).supported_layouts)(self.handle) }
    }

    pub fn output_channels(&self) -> u16 {
        let _call = self.ctx.begin_call();
        unsafe { ((*self.vtable).output_channels)(self.handle) }
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
        status_to_result("Dsp plan_config_update_json", status, self.ctx.plugin_free)?;
        Ok(plan_from_ffi(out, self.ctx.plugin_free))
    }

    pub fn apply_config_update_json(&mut self, new_config_json: &str) -> Result<()> {
        let Some(apply_fn) = (unsafe { (*self.vtable).apply_config_update_json_utf8 }) else {
            return Err(anyhow!("dsp apply_config_update not supported"));
        };
        let req = self
            .ctx
            .updates
            .enqueue(self.ctx.instance_id, new_config_json.to_string());
        let _call = self.ctx.begin_call();
        let status = (apply_fn)(self.handle, ststr_from_str(&req.config_json));
        self.ctx.updates.complete(self.ctx.instance_id);
        status_to_result("Dsp apply_config_update_json", status, self.ctx.plugin_free)
    }

    pub fn export_state_json(&self) -> Result<Option<String>> {
        let Some(export_fn) = (unsafe { (*self.vtable).export_state_json_utf8 }) else {
            return Ok(None);
        };
        let _call = self.ctx.begin_call();
        let mut out = StStr::empty();
        let status = (export_fn)(self.handle, &mut out);
        status_to_result("Dsp export_state_json", status, self.ctx.plugin_free)?;
        let raw = take_plugin_string(out, self.ctx.plugin_free);
        if raw.is_empty() {
            Ok(None)
        } else {
            Ok(Some(raw))
        }
    }

    pub fn import_state_json(&mut self, state_json: &str) -> Result<()> {
        let Some(import_fn) = (unsafe { (*self.vtable).import_state_json_utf8 }) else {
            return Err(anyhow!("dsp import_state_json not supported"));
        };
        let _call = self.ctx.begin_call();
        let status = (import_fn)(self.handle, ststr_from_str(state_json));
        status_to_result("Dsp import_state_json", status, self.ctx.plugin_free)
    }
}

impl Drop for DspInstanceV2 {
    fn drop(&mut self) {
        if !self.handle.is_null() && !self.vtable.is_null() {
            let _call = self.ctx.begin_call();
            unsafe { ((*self.vtable).destroy)(self.handle) };
            self.handle = core::ptr::null_mut();
        }
        self.ctx.unregister();
    }
}
