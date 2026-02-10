use std::sync::Arc;

use anyhow::{Result, anyhow};
use stellatune_plugin_api::{StConfigUpdateMode, StConfigUpdatePlan};
use stellatune_plugin_api::{StStatus, StStr};

use crate::runtime::{
    GenerationGuard, InstanceId, InstanceRegistry, InstanceUpdateCoordinator,
    InstanceUpdateDecision,
};

pub type PluginFreeFn =
    Option<extern "C" fn(ptr: *mut core::ffi::c_void, len: usize, align: usize)>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigUpdatePlan {
    pub mode: StConfigUpdateMode,
    pub reason: Option<String>,
}

#[derive(Clone)]
pub struct InstanceRuntimeCtx {
    pub instance_id: InstanceId,
    pub instances: Arc<InstanceRegistry>,
    pub generation: Arc<GenerationGuard>,
    pub updates: Arc<InstanceUpdateCoordinator>,
    pub plugin_free: PluginFreeFn,
}

impl InstanceRuntimeCtx {
    pub fn begin_call(&self) -> GenerationCallGuard {
        GenerationCallGuard::enter(&self.generation)
    }

    pub fn unregister(&self) {
        let _ = self.instances.remove(self.instance_id);
        self.updates.complete(self.instance_id);
    }
}

pub struct GenerationCallGuard {
    generation: Arc<GenerationGuard>,
}

impl GenerationCallGuard {
    pub fn enter(generation: &Arc<GenerationGuard>) -> Self {
        generation.inc_inflight_call();
        Self {
            generation: Arc::clone(generation),
        }
    }
}

impl Drop for GenerationCallGuard {
    fn drop(&mut self) {
        self.generation.dec_inflight_call();
    }
}

pub fn ststr_from_str(s: &str) -> StStr {
    StStr {
        ptr: s.as_ptr(),
        len: s.len(),
    }
}

pub fn status_to_result(what: &str, status: StStatus, plugin_free: PluginFreeFn) -> Result<()> {
    if status.code == 0 {
        return Ok(());
    }
    Err(status_err_to_anyhow(what, status, plugin_free))
}

fn status_err_to_anyhow(what: &str, status: StStatus, plugin_free: PluginFreeFn) -> anyhow::Error {
    let msg = unsafe { crate::util::ststr_to_string_lossy(status.message) };
    if status.code != 0
        && status.message.len != 0
        && let Some(free) = plugin_free
    {
        (free)(
            status.message.ptr as *mut core::ffi::c_void,
            status.message.len,
            1,
        );
    }
    if msg.is_empty() {
        anyhow!("{what} failed (code={})", status.code)
    } else {
        anyhow!("{what} failed (code={}): {msg}", status.code)
    }
}

pub fn take_plugin_string(s: StStr, plugin_free: PluginFreeFn) -> String {
    if s.ptr.is_null() || s.len == 0 {
        return String::new();
    }
    let text = unsafe { crate::util::ststr_to_string_lossy(s) };
    if let Some(free) = plugin_free {
        (free)(s.ptr as *mut core::ffi::c_void, s.len, 1);
    }
    text
}

pub fn plan_from_ffi(plan: StConfigUpdatePlan, plugin_free: PluginFreeFn) -> ConfigUpdatePlan {
    let reason = if plan.reason_utf8.ptr.is_null() || plan.reason_utf8.len == 0 {
        None
    } else {
        let text = take_plugin_string(plan.reason_utf8, plugin_free);
        if text.is_empty() { None } else { Some(text) }
    };
    ConfigUpdatePlan {
        mode: plan.mode,
        reason,
    }
}

pub fn decision_from_plan(plan: &ConfigUpdatePlan) -> InstanceUpdateDecision {
    match plan.mode {
        StConfigUpdateMode::HotApply => InstanceUpdateDecision::HotApply,
        StConfigUpdateMode::Recreate => InstanceUpdateDecision::Recreate,
        StConfigUpdateMode::Reject => InstanceUpdateDecision::Reject,
    }
}
