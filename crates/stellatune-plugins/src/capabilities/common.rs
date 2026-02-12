use std::sync::Arc;
#[cfg(not(debug_assertions))]
use std::sync::atomic::AtomicBool;
use std::sync::atomic::{AtomicUsize, Ordering};

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
    pub owner: Arc<InstanceCallOwner>,
    pub updates: Arc<InstanceUpdateCoordinator>,
    pub plugin_free: PluginFreeFn,
}

impl InstanceRuntimeCtx {
    pub fn begin_call(&self) -> GenerationCallGuard {
        self.owner.assert_current_thread(self.instance_id);
        GenerationCallGuard::enter(&self.generation)
    }

    pub fn unregister(&self) {
        let _ = self.instances.remove(self.instance_id);
        self.updates.complete(self.instance_id);
    }
}

pub struct InstanceCallOwner {
    owner_thread_token: AtomicUsize,
    #[cfg(not(debug_assertions))]
    mismatch_logged: AtomicBool,
}

impl InstanceCallOwner {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn assert_current_thread(&self, instance_id: InstanceId) {
        let caller = current_thread_token();
        let mut owner = self.owner_thread_token.load(Ordering::Acquire);
        if owner == 0 {
            owner = self
                .owner_thread_token
                .compare_exchange(0, caller, Ordering::AcqRel, Ordering::Acquire)
                .unwrap_or_else(|existing| existing);
            if owner == 0 {
                owner = caller;
            }
        }
        if owner == caller {
            return;
        }

        #[cfg(debug_assertions)]
        {
            panic!(
                "instance {} called from non-owner thread (owner_token={}, caller_token={})",
                instance_id.0, owner, caller
            );
        }
        #[cfg(not(debug_assertions))]
        {
            if !self.mismatch_logged.swap(true, Ordering::AcqRel) {
                tracing::error!(
                    instance_id = instance_id.0,
                    owner_thread_token = owner,
                    caller_thread_token = caller,
                    "plugin instance called from non-owner thread"
                );
            }
        }
    }
}

impl Default for InstanceCallOwner {
    fn default() -> Self {
        Self {
            owner_thread_token: AtomicUsize::new(0),
            #[cfg(not(debug_assertions))]
            mismatch_logged: AtomicBool::new(false),
        }
    }
}

thread_local! {
    static THREAD_TOKEN: u8 = const { 0 };
}

fn current_thread_token() -> usize {
    THREAD_TOKEN.with(|token| token as *const u8 as usize)
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
