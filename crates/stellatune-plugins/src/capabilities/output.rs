use anyhow::{Result, anyhow};
use stellatune_plugin_api::{StAudioSpec, StStr};
use stellatune_plugin_api::{
    StConfigUpdatePlan, StOutputSinkInstanceRef, StOutputSinkNegotiatedSpec,
    StOutputSinkRuntimeStatus,
};

use super::common::{
    ConfigUpdatePlan, InstanceRuntimeCtx, decision_from_plan, plan_from_ffi, status_to_result,
    ststr_from_str, take_plugin_string,
};

pub struct OutputSinkInstance {
    ctx: InstanceRuntimeCtx,
    handle: *mut core::ffi::c_void,
    vtable: *const stellatune_plugin_api::StOutputSinkInstanceVTable,
}

impl OutputSinkInstance {
    pub fn from_ffi(ctx: InstanceRuntimeCtx, raw: StOutputSinkInstanceRef) -> Result<Self> {
        if raw.handle.is_null() || raw.vtable.is_null() {
            return Err(anyhow!("output sink instance returned null handle/vtable"));
        }
        Ok(Self {
            ctx,
            handle: raw.handle,
            vtable: raw.vtable,
        })
    }

    pub fn instance_id(&self) -> crate::runtime::instance_registry::InstanceId {
        self.ctx.instance_id
    }

    pub fn list_targets_json(&mut self) -> Result<String> {
        let mut out = StStr::empty();
        let status = unsafe { ((*self.vtable).list_targets_json_utf8)(self.handle, &mut out) };
        status_to_result("Output list_targets_json", status, self.ctx.plugin_free)?;
        Ok(take_plugin_string(out, self.ctx.plugin_free))
    }

    pub fn negotiate_spec(
        &mut self,
        target_json: &str,
        desired_spec: StAudioSpec,
    ) -> Result<StOutputSinkNegotiatedSpec> {
        let mut out = StOutputSinkNegotiatedSpec {
            spec: StAudioSpec {
                sample_rate: 0,
                channels: 0,
                reserved: 0,
            },
            preferred_chunk_frames: 0,
            flags: 0,
            reserved: 0,
        };
        let status = unsafe {
            ((*self.vtable).negotiate_spec)(
                self.handle,
                ststr_from_str(target_json),
                desired_spec,
                &mut out,
            )
        };
        status_to_result("Output negotiate_spec", status, self.ctx.plugin_free)?;
        Ok(out)
    }

    pub fn open(&mut self, target_json: &str, spec: StAudioSpec) -> Result<()> {
        let status =
            unsafe { ((*self.vtable).open)(self.handle, ststr_from_str(target_json), spec) };
        status_to_result("Output open", status, self.ctx.plugin_free)
    }

    pub fn write_interleaved_f32(&mut self, channels: u16, samples: &[f32]) -> Result<u32> {
        let channels = channels.max(1);
        if !samples.len().is_multiple_of(channels as usize) {
            return Err(anyhow!(
                "samples length {} is not divisible by channels {}",
                samples.len(),
                channels
            ));
        }
        let frames = (samples.len() / channels as usize) as u32;
        let mut out_frames_accepted = 0u32;
        let status = unsafe {
            ((*self.vtable).write_interleaved_f32)(
                self.handle,
                frames,
                channels,
                samples.as_ptr(),
                &mut out_frames_accepted,
            )
        };
        status_to_result("Output write_interleaved_f32", status, self.ctx.plugin_free)?;
        Ok(out_frames_accepted)
    }

    pub fn query_status(&mut self) -> Result<StOutputSinkRuntimeStatus> {
        let mut out = StOutputSinkRuntimeStatus {
            queued_samples: 0,
            running: 0,
            reserved0: 0,
            reserved1: 0,
        };
        let status = unsafe { ((*self.vtable).query_status)(self.handle, &mut out) };
        status_to_result("Output query_status", status, self.ctx.plugin_free)?;
        Ok(out)
    }

    pub fn flush(&mut self) -> Result<()> {
        let Some(flush) = (unsafe { (*self.vtable).flush }) else {
            return Ok(());
        };
        let status = (flush)(self.handle);
        status_to_result("Output flush", status, self.ctx.plugin_free)
    }

    pub fn reset(&mut self) -> Result<()> {
        let status = unsafe { ((*self.vtable).reset)(self.handle) };
        status_to_result("Output reset", status, self.ctx.plugin_free)
    }

    pub fn close(&mut self) {
        unsafe { ((*self.vtable).close)(self.handle) };
    }

    fn close_before_destroy(&mut self) {
        if self.handle.is_null() || self.vtable.is_null() {
            return;
        }
        self.close();
    }

    pub fn plan_config_update_json(&self, new_config_json: &str) -> Result<ConfigUpdatePlan> {
        let Some(plan_fn) = (unsafe { (*self.vtable).plan_config_update_json_utf8 }) else {
            return Ok(ConfigUpdatePlan {
                mode: stellatune_plugin_api::StConfigUpdateMode::Recreate,
                reason: Some("plugin does not implement plan_config_update".to_string()),
            });
        };
        let mut out = StConfigUpdatePlan {
            mode: stellatune_plugin_api::StConfigUpdateMode::Reject,
            reason_utf8: StStr::empty(),
        };
        let status = (plan_fn)(self.handle, ststr_from_str(new_config_json), &mut out);
        status_to_result(
            "Output plan_config_update_json",
            status,
            self.ctx.plugin_free,
        )?;
        Ok(plan_from_ffi(out, self.ctx.plugin_free))
    }

    pub fn apply_config_update_json(
        &mut self,
        new_config_json: &str,
    ) -> Result<crate::runtime::update::InstanceUpdateResult> {
        let plan = self.plan_config_update_json(new_config_json)?;
        let decision = decision_from_plan(&plan);
        let req = self.ctx.updates.begin(
            self.ctx.instance_id,
            new_config_json.to_string(),
            decision,
            plan.reason.clone(),
        );
        match decision {
            crate::runtime::update::InstanceUpdateDecision::HotApply => {
                let Some(apply_fn) = (unsafe { (*self.vtable).apply_config_update_json_utf8 })
                else {
                    let msg = "output apply_config_update not supported".to_string();
                    let _ = self.ctx.updates.finish_failed(&req, msg.clone());
                    return Err(anyhow!(msg));
                };
                let status = (apply_fn)(self.handle, ststr_from_str(&req.config_json));
                match status_to_result(
                    "Output apply_config_update_json",
                    status,
                    self.ctx.plugin_free,
                ) {
                    Ok(()) => Ok(self.ctx.updates.finish_applied(&req)),
                    Err(err) => {
                        let _ = self.ctx.updates.finish_failed(&req, err.to_string());
                        Err(err)
                    }
                }
            }
            crate::runtime::update::InstanceUpdateDecision::Recreate => {
                Ok(self.ctx.updates.finish_requires_recreate(&req, plan.reason))
            }
            crate::runtime::update::InstanceUpdateDecision::Reject => {
                let reason = plan
                    .reason
                    .unwrap_or_else(|| "output rejected config update".to_string());
                Ok(self.ctx.updates.finish_rejected(&req, reason))
            }
        }
    }

    pub fn export_state_json(&self) -> Result<Option<String>> {
        let Some(export_fn) = (unsafe { (*self.vtable).export_state_json_utf8 }) else {
            return Ok(None);
        };
        let mut out = StStr::empty();
        let status = (export_fn)(self.handle, &mut out);
        status_to_result("Output export_state_json", status, self.ctx.plugin_free)?;
        let raw = take_plugin_string(out, self.ctx.plugin_free);
        if raw.is_empty() {
            Ok(None)
        } else {
            Ok(Some(raw))
        }
    }

    pub fn import_state_json(&mut self, state_json: &str) -> Result<()> {
        let Some(import_fn) = (unsafe { (*self.vtable).import_state_json_utf8 }) else {
            return Err(anyhow!("output import_state_json not supported"));
        };
        let status = (import_fn)(self.handle, ststr_from_str(state_json));
        status_to_result("Output import_state_json", status, self.ctx.plugin_free)
    }
}

impl Drop for OutputSinkInstance {
    fn drop(&mut self) {
        if !self.handle.is_null() && !self.vtable.is_null() {
            self.close_before_destroy();
            unsafe { ((*self.vtable).destroy)(self.handle) };
            self.handle = core::ptr::null_mut();
        }
    }
}

// SAFETY: Instances are moved into worker-owned threads and must be used from one
// owner thread at a time. Runtime call sites enforce thread ownership checks.
unsafe impl Send for OutputSinkInstance {}
