use std::marker::PhantomData;
use std::rc::Rc;

use anyhow::{Result, anyhow};
use stellatune_plugin_api::StStr;
use stellatune_plugin_api::{StConfigUpdatePlan, StDspInstanceRef};

use super::common::{
    ConfigUpdatePlan, InstanceRuntimeCtx, decision_from_plan, plan_from_ffi, status_to_result,
    ststr_from_str, take_plugin_string,
};

pub struct DspInstance {
    ctx: InstanceRuntimeCtx,
    handle: *mut core::ffi::c_void,
    vtable: *const stellatune_plugin_api::StDspInstanceVTable,
    _not_send_sync: PhantomData<Rc<()>>,
}

impl DspInstance {
    pub fn from_ffi(ctx: InstanceRuntimeCtx, raw: StDspInstanceRef) -> Result<Self> {
        if raw.handle.is_null() || raw.vtable.is_null() {
            return Err(anyhow!("dsp instance returned null handle/vtable"));
        }
        Ok(Self {
            ctx,
            handle: raw.handle,
            vtable: raw.vtable,
            _not_send_sync: PhantomData,
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
        status_to_result("Dsp plan_config_update_json", status, self.ctx.plugin_free)?;
        Ok(plan_from_ffi(out, self.ctx.plugin_free))
    }

    pub fn apply_config_update_json(
        &mut self,
        new_config_json: &str,
    ) -> Result<crate::runtime::InstanceUpdateResult> {
        let plan = self.plan_config_update_json(new_config_json)?;
        let decision = decision_from_plan(&plan);
        let req = self.ctx.updates.begin(
            self.ctx.instance_id,
            new_config_json.to_string(),
            decision,
            plan.reason.clone(),
        );
        match decision {
            crate::runtime::InstanceUpdateDecision::HotApply => {
                let Some(apply_fn) = (unsafe { (*self.vtable).apply_config_update_json_utf8 })
                else {
                    let msg = "dsp apply_config_update not supported".to_string();
                    let _ = self.ctx.updates.finish_failed(&req, msg.clone());
                    return Err(anyhow!(msg));
                };
                let _call = self.ctx.begin_call();
                let status = (apply_fn)(self.handle, ststr_from_str(&req.config_json));
                match status_to_result("Dsp apply_config_update_json", status, self.ctx.plugin_free)
                {
                    Ok(()) => Ok(self.ctx.updates.finish_applied(&req)),
                    Err(err) => {
                        let _ = self.ctx.updates.finish_failed(&req, err.to_string());
                        Err(err)
                    }
                }
            }
            crate::runtime::InstanceUpdateDecision::Recreate => {
                Ok(self.ctx.updates.finish_requires_recreate(&req, plan.reason))
            }
            crate::runtime::InstanceUpdateDecision::Reject => {
                let reason = plan
                    .reason
                    .unwrap_or_else(|| "dsp rejected config update".to_string());
                Ok(self.ctx.updates.finish_rejected(&req, reason))
            }
        }
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

impl Drop for DspInstance {
    fn drop(&mut self) {
        if !self.handle.is_null() && !self.vtable.is_null() {
            let _call = self.ctx.begin_call();
            unsafe { ((*self.vtable).destroy)(self.handle) };
            self.handle = core::ptr::null_mut();
        }
        self.ctx.unregister();
    }
}
