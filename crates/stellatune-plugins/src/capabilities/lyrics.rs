use anyhow::{Result, anyhow};
use std::ffi::c_void;
use std::ptr;
use std::time::{Duration, Instant};
use stellatune_plugin_api::{
    StAsyncOpState, StConfigUpdateMode, StConfigUpdatePlan, StConfigUpdatePlanOpRef, StJsonOpRef,
    StLyricsJsonOpRef, StLyricsProviderInstanceRef, StLyricsProviderInstanceVTable, StStr,
    StUnitOpRef,
};

use super::common::{
    ConfigUpdatePlan, InstanceRuntimeCtx, PluginFreeFn, decision_from_plan, plan_from_ffi,
    status_to_result, ststr_from_str, take_plugin_string,
};

const LYRICS_OP_WAIT_SLICE_MS: u32 = 250;
const LYRICS_CONTROL_TIMEOUT: Duration = Duration::from_secs(20);
const LYRICS_UNIT_TIMEOUT: Duration = Duration::from_secs(10);
const LYRICS_JSON_TIMEOUT: Duration = Duration::from_secs(10);
const LYRICS_PLAN_TIMEOUT: Duration = Duration::from_secs(10);

pub struct LyricsProviderInstance {
    ctx: InstanceRuntimeCtx,
    handle: *mut c_void,
    vtable: *const StLyricsProviderInstanceVTable,
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
        let mut op = StLyricsJsonOpRef {
            handle: ptr::null_mut(),
            vtable: ptr::null(),
            reserved0: 0,
            reserved1: 0,
        };
        let status = unsafe {
            ((*self.vtable).begin_search_json_utf8)(
                self.handle,
                ststr_from_str(query_json),
                &mut op,
            )
        };
        status_to_result("Lyrics begin_search_json", status, self.ctx.plugin_free)?;
        if op.handle.is_null() || op.vtable.is_null() {
            return Err(anyhow!(
                "lyrics begin_search_json returned null op handle/vtable"
            ));
        }
        let result = (|| match wait_lyrics_op_state(&op, self.ctx.plugin_free)? {
            StAsyncOpState::Ready => {
                let mut out = StStr::empty();
                let status = unsafe { ((*op.vtable).take_json_utf8)(op.handle, &mut out) };
                status_to_result("Lyrics search take_json_utf8", status, self.ctx.plugin_free)?;
                Ok(take_plugin_string(out, self.ctx.plugin_free))
            },
            StAsyncOpState::Cancelled => Err(anyhow!("lyrics search operation cancelled")),
            StAsyncOpState::Failed => {
                let mut out = StStr::empty();
                let status = unsafe { ((*op.vtable).take_json_utf8)(op.handle, &mut out) };
                status_to_result("Lyrics search op failed", status, self.ctx.plugin_free)?;
                Err(anyhow!("lyrics search operation failed"))
            },
            StAsyncOpState::Pending => Err(anyhow!("lyrics search operation still pending")),
        })();
        unsafe { ((*op.vtable).destroy)(op.handle) };
        result
    }

    pub fn fetch_json(&mut self, track_json: &str) -> Result<String> {
        let mut op = StLyricsJsonOpRef {
            handle: ptr::null_mut(),
            vtable: ptr::null(),
            reserved0: 0,
            reserved1: 0,
        };
        let status = unsafe {
            ((*self.vtable).begin_fetch_json_utf8)(self.handle, ststr_from_str(track_json), &mut op)
        };
        status_to_result("Lyrics begin_fetch_json", status, self.ctx.plugin_free)?;
        if op.handle.is_null() || op.vtable.is_null() {
            return Err(anyhow!(
                "lyrics begin_fetch_json returned null op handle/vtable"
            ));
        }
        let result = (|| match wait_lyrics_op_state(&op, self.ctx.plugin_free)? {
            StAsyncOpState::Ready => {
                let mut out = StStr::empty();
                let status = unsafe { ((*op.vtable).take_json_utf8)(op.handle, &mut out) };
                status_to_result("Lyrics fetch take_json_utf8", status, self.ctx.plugin_free)?;
                Ok(take_plugin_string(out, self.ctx.plugin_free))
            },
            StAsyncOpState::Cancelled => Err(anyhow!("lyrics fetch operation cancelled")),
            StAsyncOpState::Failed => {
                let mut out = StStr::empty();
                let status = unsafe { ((*op.vtable).take_json_utf8)(op.handle, &mut out) };
                status_to_result("Lyrics fetch op failed", status, self.ctx.plugin_free)?;
                Err(anyhow!("lyrics fetch operation failed"))
            },
            StAsyncOpState::Pending => Err(anyhow!("lyrics fetch operation still pending")),
        })();
        unsafe { ((*op.vtable).destroy)(op.handle) };
        result
    }

    pub fn plan_config_update_json(&self, new_config_json: &str) -> Result<ConfigUpdatePlan> {
        let Some(plan_fn) = (unsafe { (*self.vtable).begin_plan_config_update_json_utf8 }) else {
            return Ok(ConfigUpdatePlan {
                mode: StConfigUpdateMode::Recreate,
                reason: Some("plugin does not implement plan_config_update".to_string()),
            });
        };

        let mut op = StConfigUpdatePlanOpRef {
            handle: ptr::null_mut(),
            vtable: ptr::null(),
            reserved0: 0,
            reserved1: 0,
        };
        let status = (plan_fn)(self.handle, ststr_from_str(new_config_json), &mut op);
        status_to_result(
            "Lyrics begin_plan_config_update_json",
            status,
            self.ctx.plugin_free,
        )?;
        if op.handle.is_null() || op.vtable.is_null() {
            return Err(anyhow!(
                "lyrics begin_plan_config_update_json returned null op handle/vtable"
            ));
        }

        let result = (|| match wait_plan_op_state(&op, self.ctx.plugin_free)? {
            StAsyncOpState::Ready => {
                let mut out = StConfigUpdatePlan {
                    mode: StConfigUpdateMode::Reject,
                    reason_utf8: StStr::empty(),
                };
                let status = unsafe { ((*op.vtable).take_plan)(op.handle, &mut out) };
                status_to_result(
                    "Lyrics plan_config_update_json",
                    status,
                    self.ctx.plugin_free,
                )?;
                Ok(plan_from_ffi(out, self.ctx.plugin_free))
            },
            StAsyncOpState::Cancelled => {
                Err(anyhow!("lyrics plan_config_update operation cancelled"))
            },
            StAsyncOpState::Failed => {
                let mut out = StConfigUpdatePlan {
                    mode: StConfigUpdateMode::Reject,
                    reason_utf8: StStr::empty(),
                };
                let status = unsafe { ((*op.vtable).take_plan)(op.handle, &mut out) };
                status_to_result(
                    "Lyrics plan_config_update op failed",
                    status,
                    self.ctx.plugin_free,
                )?;
                Err(anyhow!("lyrics plan_config_update operation failed"))
            },
            StAsyncOpState::Pending => {
                Err(anyhow!("lyrics plan_config_update operation still pending"))
            },
        })();

        unsafe { ((*op.vtable).destroy)(op.handle) };
        result
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
                let Some(apply_fn) =
                    (unsafe { (*self.vtable).begin_apply_config_update_json_utf8 })
                else {
                    let msg = "lyrics apply_config_update not supported".to_string();
                    let _ = self.ctx.updates.finish_failed(&req, msg.clone());
                    return Err(anyhow!(msg));
                };

                let mut op = StUnitOpRef {
                    handle: ptr::null_mut(),
                    vtable: ptr::null(),
                    reserved0: 0,
                    reserved1: 0,
                };
                let status = (apply_fn)(self.handle, ststr_from_str(&req.config_json), &mut op);
                let apply_res = (|| {
                    status_to_result(
                        "Lyrics begin_apply_config_update_json",
                        status,
                        self.ctx.plugin_free,
                    )?;
                    if op.handle.is_null() || op.vtable.is_null() {
                        return Err(anyhow!(
                            "lyrics begin_apply_config_update returned null op handle/vtable"
                        ));
                    }
                    match wait_unit_op_state(
                        &op,
                        self.ctx.plugin_free,
                        "Lyrics apply_config_update op wait",
                    )? {
                        StAsyncOpState::Ready => {
                            let status = unsafe { ((*op.vtable).finish)(op.handle) };
                            status_to_result(
                                "Lyrics apply_config_update_json",
                                status,
                                self.ctx.plugin_free,
                            )
                        },
                        StAsyncOpState::Cancelled => {
                            Err(anyhow!("lyrics apply_config_update operation cancelled"))
                        },
                        StAsyncOpState::Failed => {
                            let status = unsafe { ((*op.vtable).finish)(op.handle) };
                            status_to_result(
                                "Lyrics apply_config_update op failed",
                                status,
                                self.ctx.plugin_free,
                            )?;
                            Err(anyhow!("lyrics apply_config_update operation failed"))
                        },
                        StAsyncOpState::Pending => Err(anyhow!(
                            "lyrics apply_config_update operation still pending"
                        )),
                    }
                })();
                if !op.handle.is_null() && !op.vtable.is_null() {
                    unsafe { ((*op.vtable).destroy)(op.handle) };
                }

                match apply_res {
                    Ok(()) => Ok(self.ctx.updates.finish_applied(&req)),
                    Err(err) => {
                        let _ = self.ctx.updates.finish_failed(&req, err.to_string());
                        Err(err)
                    },
                }
            },
            crate::runtime::update::InstanceUpdateDecision::Recreate => {
                Ok(self.ctx.updates.finish_requires_recreate(&req, plan.reason))
            },
            crate::runtime::update::InstanceUpdateDecision::Reject => {
                let reason = plan
                    .reason
                    .unwrap_or_else(|| "lyrics rejected config update".to_string());
                Ok(self.ctx.updates.finish_rejected(&req, reason))
            },
        }
    }

    pub fn export_state_json(&self) -> Result<Option<String>> {
        let Some(export_fn) = (unsafe { (*self.vtable).begin_export_state_json_utf8 }) else {
            return Ok(None);
        };

        let mut op = StJsonOpRef {
            handle: ptr::null_mut(),
            vtable: ptr::null(),
            reserved0: 0,
            reserved1: 0,
        };
        let status = (export_fn)(self.handle, &mut op);
        status_to_result(
            "Lyrics begin_export_state_json",
            status,
            self.ctx.plugin_free,
        )?;
        if op.handle.is_null() || op.vtable.is_null() {
            return Err(anyhow!(
                "lyrics begin_export_state_json returned null op handle/vtable"
            ));
        }

        let result = (|| match wait_json_op_state(&op, self.ctx.plugin_free)? {
            StAsyncOpState::Ready => {
                let mut out = StStr::empty();
                let status = unsafe { ((*op.vtable).take_json_utf8)(op.handle, &mut out) };
                status_to_result("Lyrics export_state_json", status, self.ctx.plugin_free)?;
                let raw = take_plugin_string(out, self.ctx.plugin_free);
                if raw.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(raw))
                }
            },
            StAsyncOpState::Cancelled => Err(anyhow!("lyrics export_state operation cancelled")),
            StAsyncOpState::Failed => {
                let mut out = StStr::empty();
                let status = unsafe { ((*op.vtable).take_json_utf8)(op.handle, &mut out) };
                status_to_result(
                    "Lyrics export_state op failed",
                    status,
                    self.ctx.plugin_free,
                )?;
                Err(anyhow!("lyrics export_state operation failed"))
            },
            StAsyncOpState::Pending => Err(anyhow!("lyrics export_state operation still pending")),
        })();

        unsafe { ((*op.vtable).destroy)(op.handle) };
        result
    }

    pub fn import_state_json(&mut self, state_json: &str) -> Result<()> {
        let Some(import_fn) = (unsafe { (*self.vtable).begin_import_state_json_utf8 }) else {
            return Err(anyhow!("lyrics import_state_json not supported"));
        };

        let mut op = StUnitOpRef {
            handle: ptr::null_mut(),
            vtable: ptr::null(),
            reserved0: 0,
            reserved1: 0,
        };
        let status = (import_fn)(self.handle, ststr_from_str(state_json), &mut op);
        status_to_result(
            "Lyrics begin_import_state_json",
            status,
            self.ctx.plugin_free,
        )?;
        if op.handle.is_null() || op.vtable.is_null() {
            return Err(anyhow!(
                "lyrics begin_import_state_json returned null op handle/vtable"
            ));
        }

        let result =
            (|| match wait_unit_op_state(&op, self.ctx.plugin_free, "Lyrics import_state op wait")?
            {
                StAsyncOpState::Ready => {
                    let status = unsafe { ((*op.vtable).finish)(op.handle) };
                    status_to_result("Lyrics import_state_json", status, self.ctx.plugin_free)
                },
                StAsyncOpState::Cancelled => {
                    Err(anyhow!("lyrics import_state operation cancelled"))
                },
                StAsyncOpState::Failed => {
                    let status = unsafe { ((*op.vtable).finish)(op.handle) };
                    status_to_result(
                        "Lyrics import_state op failed",
                        status,
                        self.ctx.plugin_free,
                    )?;
                    Err(anyhow!("lyrics import_state operation failed"))
                },
                StAsyncOpState::Pending => {
                    Err(anyhow!("lyrics import_state operation still pending"))
                },
            })();

        unsafe { ((*op.vtable).destroy)(op.handle) };
        result
    }
}

fn wait_lyrics_op_state(
    op: &StLyricsJsonOpRef,
    plugin_free: PluginFreeFn,
) -> Result<StAsyncOpState> {
    let started = Instant::now();
    loop {
        let mut state = StAsyncOpState::Pending;
        let status = unsafe { ((*op.vtable).wait)(op.handle, LYRICS_OP_WAIT_SLICE_MS, &mut state) };
        status_to_result("Lyrics op wait", status, plugin_free)?;
        if state != StAsyncOpState::Pending {
            return Ok(state);
        }
        if started.elapsed() >= LYRICS_CONTROL_TIMEOUT {
            let _ = status_to_result(
                "Lyrics op cancel",
                unsafe { ((*op.vtable).cancel)(op.handle) },
                plugin_free,
            );
            return Err(anyhow!(
                "lyrics operation timed out after {}ms",
                LYRICS_CONTROL_TIMEOUT.as_millis()
            ));
        }
    }
}

fn wait_unit_op_state(
    op: &StUnitOpRef,
    plugin_free: PluginFreeFn,
    what: &str,
) -> Result<StAsyncOpState> {
    let started = Instant::now();
    loop {
        let mut state = StAsyncOpState::Pending;
        let status = unsafe { ((*op.vtable).wait)(op.handle, LYRICS_OP_WAIT_SLICE_MS, &mut state) };
        status_to_result(what, status, plugin_free)?;
        if state != StAsyncOpState::Pending {
            return Ok(state);
        }
        if started.elapsed() >= LYRICS_UNIT_TIMEOUT {
            let _ = status_to_result(
                "Lyrics unit op cancel",
                unsafe { ((*op.vtable).cancel)(op.handle) },
                plugin_free,
            );
            return Err(anyhow!(
                "lyrics unit operation timed out after {}ms",
                LYRICS_UNIT_TIMEOUT.as_millis()
            ));
        }
    }
}

fn wait_json_op_state(op: &StJsonOpRef, plugin_free: PluginFreeFn) -> Result<StAsyncOpState> {
    let started = Instant::now();
    loop {
        let mut state = StAsyncOpState::Pending;
        let status = unsafe { ((*op.vtable).wait)(op.handle, LYRICS_OP_WAIT_SLICE_MS, &mut state) };
        status_to_result("Lyrics json op wait", status, plugin_free)?;
        if state != StAsyncOpState::Pending {
            return Ok(state);
        }
        if started.elapsed() >= LYRICS_JSON_TIMEOUT {
            let _ = status_to_result(
                "Lyrics json op cancel",
                unsafe { ((*op.vtable).cancel)(op.handle) },
                plugin_free,
            );
            return Err(anyhow!(
                "lyrics json operation timed out after {}ms",
                LYRICS_JSON_TIMEOUT.as_millis()
            ));
        }
    }
}

fn wait_plan_op_state(
    op: &StConfigUpdatePlanOpRef,
    plugin_free: PluginFreeFn,
) -> Result<StAsyncOpState> {
    let started = Instant::now();
    loop {
        let mut state = StAsyncOpState::Pending;
        let status = unsafe { ((*op.vtable).wait)(op.handle, LYRICS_OP_WAIT_SLICE_MS, &mut state) };
        status_to_result("Lyrics plan op wait", status, plugin_free)?;
        if state != StAsyncOpState::Pending {
            return Ok(state);
        }
        if started.elapsed() >= LYRICS_PLAN_TIMEOUT {
            let _ = status_to_result(
                "Lyrics plan op cancel",
                unsafe { ((*op.vtable).cancel)(op.handle) },
                plugin_free,
            );
            return Err(anyhow!(
                "lyrics plan operation timed out after {}ms",
                LYRICS_PLAN_TIMEOUT.as_millis()
            ));
        }
    }
}

impl Drop for LyricsProviderInstance {
    fn drop(&mut self) {
        if !self.handle.is_null() && !self.vtable.is_null() {
            unsafe { ((*self.vtable).destroy)(self.handle) };
            self.handle = ptr::null_mut();
        }
    }
}

// SAFETY: The worker model requires moving instances across thread boundaries and
// using each instance from exactly one worker thread at a time.
unsafe impl Send for LyricsProviderInstance {}
