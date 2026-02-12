use anyhow::{Result, anyhow};
use stellatune_plugin_api::StStr;
use stellatune_plugin_api::{StConfigUpdatePlan, StLyricsProviderInstanceRef};

use super::common::{
    ConfigUpdatePlan, InstanceRuntimeCtx, decision_from_plan, plan_from_ffi, status_to_result,
    ststr_from_str, take_plugin_string,
};

pub struct LyricsProviderInstance {
    ctx: InstanceRuntimeCtx,
    handle: *mut core::ffi::c_void,
    vtable: *const stellatune_plugin_api::StLyricsProviderInstanceVTable,
}

impl LyricsProviderInstance {
    pub fn from_ffi(ctx: InstanceRuntimeCtx, raw: StLyricsProviderInstanceRef) -> Result<Self> {
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

    pub fn instance_id(&self) -> crate::runtime::instance_registry::InstanceId {
        self.ctx.instance_id
    }

    pub fn search_json(&mut self, query_json: &str) -> Result<String> {
        let mut out = StStr::empty();
        let status = unsafe {
            ((*self.vtable).search_json_utf8)(self.handle, ststr_from_str(query_json), &mut out)
        };
        status_to_result("Lyrics search_json", status, self.ctx.plugin_free)?;
        Ok(take_plugin_string(out, self.ctx.plugin_free))
    }

    pub fn fetch_json(&mut self, track_json: &str) -> Result<String> {
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
            "Lyrics plan_config_update_json",
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
                    let msg = "lyrics apply_config_update not supported".to_string();
                    let _ = self.ctx.updates.finish_failed(&req, msg.clone());
                    return Err(anyhow!(msg));
                };
                let status = (apply_fn)(self.handle, ststr_from_str(&req.config_json));
                match status_to_result(
                    "Lyrics apply_config_update_json",
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
                    .unwrap_or_else(|| "lyrics rejected config update".to_string());
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
        let status = (import_fn)(self.handle, ststr_from_str(state_json));
        status_to_result("Lyrics import_state_json", status, self.ctx.plugin_free)
    }
}

impl Drop for LyricsProviderInstance {
    fn drop(&mut self) {
        if !self.handle.is_null() && !self.vtable.is_null() {
            unsafe { ((*self.vtable).destroy)(self.handle) };
            self.handle = core::ptr::null_mut();
        }
    }
}

// SAFETY: Instances are moved into worker-owned threads and must be used from one
// owner thread at a time. Runtime call sites enforce thread ownership checks.
unsafe impl Send for LyricsProviderInstance {}
