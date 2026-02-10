use anyhow::{Result, anyhow};
use stellatune_plugin_api::{StConfigUpdatePlan, StDecoderInstanceRef, StDecoderOpenArgs};
use stellatune_plugin_api::{StDecoderInfo, StIoVTable, StStr};

use super::common::{
    ConfigUpdatePlan, InstanceRuntimeCtx, decision_from_plan, plan_from_ffi, status_to_result,
    ststr_from_str, take_plugin_string,
};

pub struct DecoderInstance {
    ctx: InstanceRuntimeCtx,
    handle: *mut core::ffi::c_void,
    vtable: *const stellatune_plugin_api::StDecoderInstanceVTable,
}

unsafe impl Send for DecoderInstance {}

impl DecoderInstance {
    pub fn from_ffi(ctx: InstanceRuntimeCtx, raw: StDecoderInstanceRef) -> Result<Self> {
        if raw.handle.is_null() || raw.vtable.is_null() {
            return Err(anyhow!("decoder instance returned null handle/vtable"));
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

    pub fn open_with_io(
        &mut self,
        path_hint: &str,
        ext_hint: &str,
        io_vtable: *const StIoVTable,
        io_handle: *mut core::ffi::c_void,
    ) -> Result<()> {
        if io_vtable.is_null() || io_handle.is_null() {
            return Err(anyhow!(
                "decoder open_with_io received null io_vtable/io_handle"
            ));
        }
        let args = StDecoderOpenArgs {
            path_utf8: ststr_from_str(path_hint),
            ext_utf8: ststr_from_str(ext_hint),
            io_vtable,
            io_handle,
        };
        let _call = self.ctx.begin_call();
        let status = unsafe { ((*self.vtable).open)(self.handle, args) };
        status_to_result("Decoder open_with_io", status, self.ctx.plugin_free)
    }

    pub fn get_info(&self) -> Result<StDecoderInfo> {
        let _call = self.ctx.begin_call();
        let mut out = StDecoderInfo {
            spec: stellatune_plugin_api::StAudioSpec {
                sample_rate: 0,
                channels: 0,
                reserved: 0,
            },
            duration_ms: 0,
            flags: 0,
            reserved: 0,
        };
        let status = unsafe { ((*self.vtable).get_info)(self.handle, &mut out) };
        status_to_result("Decoder get_info", status, self.ctx.plugin_free)?;
        Ok(out)
    }

    pub fn get_metadata_json(&self) -> Result<Option<String>> {
        let Some(get) = (unsafe { (*self.vtable).get_metadata_json_utf8 }) else {
            return Ok(None);
        };
        let _call = self.ctx.begin_call();
        let mut out = StStr::empty();
        let status = (get)(self.handle, &mut out);
        status_to_result("Decoder get_metadata_json", status, self.ctx.plugin_free)?;
        let raw = take_plugin_string(out, self.ctx.plugin_free);
        if raw.is_empty() {
            Ok(None)
        } else {
            Ok(Some(raw))
        }
    }

    pub fn read_interleaved_f32(&mut self, frames: u32) -> Result<(Vec<f32>, u32, bool)> {
        let info = self.get_info()?;
        let channels = info.spec.channels.max(1) as usize;
        let mut out = vec![0.0f32; (frames as usize).saturating_mul(channels)];
        let mut frames_read = 0u32;
        let mut eof = false;
        let _call = self.ctx.begin_call();
        let status = unsafe {
            ((*self.vtable).read_interleaved_f32)(
                self.handle,
                frames,
                out.as_mut_ptr(),
                &mut frames_read,
                &mut eof,
            )
        };
        status_to_result("Decoder read_interleaved_f32", status, self.ctx.plugin_free)?;
        out.truncate((frames_read as usize).saturating_mul(channels));
        Ok((out, frames_read, eof))
    }

    pub fn seek_ms(&mut self, position_ms: u64) -> Result<()> {
        let Some(seek) = (unsafe { (*self.vtable).seek_ms }) else {
            return Err(anyhow!("decoder seek not supported"));
        };
        let _call = self.ctx.begin_call();
        let status = (seek)(self.handle, position_ms);
        status_to_result("Decoder seek_ms", status, self.ctx.plugin_free)
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
            "Decoder plan_config_update_json",
            status,
            self.ctx.plugin_free,
        )?;
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
                    let msg = "decoder apply_config_update not supported".to_string();
                    let _ = self.ctx.updates.finish_failed(&req, msg.clone());
                    return Err(anyhow!(msg));
                };
                let _call = self.ctx.begin_call();
                let status = (apply_fn)(self.handle, ststr_from_str(&req.config_json));
                match status_to_result(
                    "Decoder apply_config_update_json",
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
            crate::runtime::InstanceUpdateDecision::Recreate => {
                Ok(self.ctx.updates.finish_requires_recreate(&req, plan.reason))
            }
            crate::runtime::InstanceUpdateDecision::Reject => {
                let reason = plan
                    .reason
                    .unwrap_or_else(|| "decoder rejected config update".to_string());
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
        status_to_result("Decoder export_state_json", status, self.ctx.plugin_free)?;
        let raw = take_plugin_string(out, self.ctx.plugin_free);
        if raw.is_empty() {
            Ok(None)
        } else {
            Ok(Some(raw))
        }
    }

    pub fn import_state_json(&mut self, state_json: &str) -> Result<()> {
        let Some(import_fn) = (unsafe { (*self.vtable).import_state_json_utf8 }) else {
            return Err(anyhow!("decoder import_state_json not supported"));
        };
        let _call = self.ctx.begin_call();
        let status = (import_fn)(self.handle, ststr_from_str(state_json));
        status_to_result("Decoder import_state_json", status, self.ctx.plugin_free)
    }
}

impl Drop for DecoderInstance {
    fn drop(&mut self) {
        if !self.handle.is_null() && !self.vtable.is_null() {
            let _call = self.ctx.begin_call();
            unsafe { ((*self.vtable).destroy)(self.handle) };
            self.handle = core::ptr::null_mut();
        }
        self.ctx.unregister();
    }
}
