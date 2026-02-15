use std::ffi::c_void;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll, Waker};
use std::time::Instant;
use std::{sync::Mutex, time::Duration};

use anyhow::{Result, anyhow};
use stellatune_plugin_api::{
    StAsyncOpState, StConfigUpdateMode, StConfigUpdatePlan, StConfigUpdatePlanOpRef, StIoVTable,
    StJsonOpRef, StOpNotifier, StOpNotifyFn, StSourceCatalogInstanceRef,
    StSourceCatalogInstanceVTable, StSourceListItemsOpRef, StSourceListItemsOpVTable,
    StSourceOpenStreamOpRef, StSourceOpenStreamOpVTable, StStatus, StStr, StUnitOpRef,
};

use super::common::{
    ConfigUpdatePlan, InstanceRuntimeCtx, PluginFreeFn, decision_from_plan, plan_from_ffi,
    status_to_result, ststr_from_str, take_plugin_string,
};

const SOURCE_OP_WAIT_SLICE_MS: u32 = 250;
const SOURCE_LIST_TIMEOUT: Duration = Duration::from_secs(20);
const SOURCE_OPEN_TIMEOUT: Duration = Duration::from_secs(20);
const SOURCE_UNIT_TIMEOUT: Duration = Duration::from_secs(10);
const SOURCE_JSON_TIMEOUT: Duration = Duration::from_secs(10);
const SOURCE_PLAN_TIMEOUT: Duration = Duration::from_secs(10);

pub struct SourceCatalogInstance {
    ctx: InstanceRuntimeCtx,
    handle: *mut c_void,
    vtable: *const StSourceCatalogInstanceVTable,
}

#[derive(Debug, Clone, Copy)]
pub struct SourceOpenStreamResult {
    pub io_vtable: *const StIoVTable,
    pub io_handle: *mut c_void,
}

type OpPollFn = extern "C" fn(handle: *mut c_void, out_state: *mut StAsyncOpState) -> StStatus;
type OpCancelFn = extern "C" fn(handle: *mut c_void) -> StStatus;
type OpSetNotifierFn = extern "C" fn(handle: *mut c_void, notifier: StOpNotifier) -> StStatus;

#[derive(Default)]
struct OpNotifyState {
    fired: AtomicBool,
    waker: Mutex<Option<Waker>>,
}

impl OpNotifyState {
    fn register_waker(&self, waker: &Waker) {
        if let Ok(mut slot) = self.waker.lock() {
            *slot = Some(waker.clone());
        }
    }

    fn take_fired(&self) -> bool {
        self.fired.swap(false, Ordering::AcqRel)
    }
}

extern "C" fn abi_op_notify_callback(user_data: *mut c_void) {
    if user_data.is_null() {
        return;
    }
    let ptr = user_data as *const OpNotifyState;
    let state = unsafe { Arc::<OpNotifyState>::from_raw(ptr) };
    state.fired.store(true, Ordering::Release);
    let wake = state
        .waker
        .lock()
        .ok()
        .and_then(|slot| slot.as_ref().cloned());
    if let Some(waker) = wake {
        waker.wake();
    }
    let _ = Arc::into_raw(state);
}

struct AbiOpWaitFuture {
    handle_addr: usize,
    poll_fn: OpPollFn,
    cancel_fn: OpCancelFn,
    set_notifier_fn: OpSetNotifierFn,
    plugin_free: PluginFreeFn,
    timeout_at: Instant,
    timeout_message: &'static str,
    notifier_installed: bool,
    notifier_token_addr: usize,
    notify_state: Arc<OpNotifyState>,
}

impl AbiOpWaitFuture {
    fn new(
        handle_addr: usize,
        poll_fn: OpPollFn,
        cancel_fn: OpCancelFn,
        set_notifier_fn: OpSetNotifierFn,
        plugin_free: PluginFreeFn,
        timeout: Duration,
        timeout_message: &'static str,
    ) -> Self {
        Self {
            handle_addr,
            poll_fn,
            cancel_fn,
            set_notifier_fn,
            plugin_free,
            timeout_at: Instant::now() + timeout,
            timeout_message,
            notifier_installed: false,
            notifier_token_addr: 0,
            notify_state: Arc::new(OpNotifyState::default()),
        }
    }

    fn handle_ptr(&self) -> *mut c_void {
        self.handle_addr as *mut c_void
    }
}

impl Future for AbiOpWaitFuture {
    type Output = Result<StAsyncOpState>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if !self.notifier_installed {
            let token_addr = Arc::into_raw(Arc::clone(&self.notify_state)) as usize;
            let status = (self.set_notifier_fn)(
                self.handle_ptr(),
                StOpNotifier {
                    user_data: token_addr as *mut c_void,
                    notify: Some(abi_op_notify_callback as StOpNotifyFn),
                },
            );
            if let Err(err) = status_to_result("Source op set_notifier", status, self.plugin_free) {
                unsafe {
                    drop(Arc::from_raw(token_addr as *const OpNotifyState));
                }
                return Poll::Ready(Err(err));
            }
            self.notifier_token_addr = token_addr;
            self.notifier_installed = true;
        }

        let mut state = StAsyncOpState::Pending;
        let status = (self.poll_fn)(self.handle_ptr(), &mut state);
        if let Err(err) = status_to_result("Source op poll", status, self.plugin_free) {
            return Poll::Ready(Err(err));
        }
        if state != StAsyncOpState::Pending {
            return Poll::Ready(Ok(state));
        }

        if Instant::now() >= self.timeout_at {
            let _ = status_to_result(
                "Source op cancel",
                (self.cancel_fn)(self.handle_ptr()),
                self.plugin_free,
            );
            return Poll::Ready(Err(anyhow!(self.timeout_message)));
        }

        self.notify_state.register_waker(cx.waker());
        if self.notify_state.take_fired() {
            cx.waker().wake_by_ref();
        }
        Poll::Pending
    }
}

impl Drop for AbiOpWaitFuture {
    fn drop(&mut self) {
        if self.notifier_token_addr != 0 {
            unsafe {
                drop(Arc::from_raw(
                    self.notifier_token_addr as *const OpNotifyState,
                ));
            }
            self.notifier_token_addr = 0;
        }
    }
}

struct SourceListItemsOpOwned {
    handle_addr: usize,
    vtable_addr: usize,
}

impl SourceListItemsOpOwned {
    fn from_raw(raw: StSourceListItemsOpRef) -> Self {
        Self {
            handle_addr: raw.handle as usize,
            vtable_addr: raw.vtable as usize,
        }
    }

    fn handle_ptr(&self) -> *mut c_void {
        self.handle_addr as *mut c_void
    }

    fn vtable(&self) -> &StSourceListItemsOpVTable {
        unsafe { &*(self.vtable_addr as *const StSourceListItemsOpVTable) }
    }

    async fn wait_ready_async(
        &self,
        plugin_free: PluginFreeFn,
        timeout: Duration,
        timeout_message: &'static str,
    ) -> Result<StAsyncOpState> {
        AbiOpWaitFuture::new(
            self.handle_addr,
            self.vtable().poll,
            self.vtable().cancel,
            self.vtable().set_notifier,
            plugin_free,
            timeout,
            timeout_message,
        )
        .await
    }

    fn take_json_utf8(&mut self, plugin_free: PluginFreeFn) -> Result<String> {
        let mut out = StStr::empty();
        let status = (self.vtable().take_json_utf8)(self.handle_ptr(), &mut out);
        status_to_result("Source list_items take_json_utf8", status, plugin_free)?;
        Ok(take_plugin_string(out, plugin_free))
    }
}

impl Drop for SourceListItemsOpOwned {
    fn drop(&mut self) {
        if self.handle_addr != 0 && self.vtable_addr != 0 {
            (self.vtable().destroy)(self.handle_ptr());
            self.handle_addr = 0;
        }
    }
}

struct SourceOpenStreamOpOwned {
    handle_addr: usize,
    vtable_addr: usize,
}

impl SourceOpenStreamOpOwned {
    fn from_raw(raw: StSourceOpenStreamOpRef) -> Self {
        Self {
            handle_addr: raw.handle as usize,
            vtable_addr: raw.vtable as usize,
        }
    }

    fn handle_ptr(&self) -> *mut c_void {
        self.handle_addr as *mut c_void
    }

    fn vtable(&self) -> &StSourceOpenStreamOpVTable {
        unsafe { &*(self.vtable_addr as *const StSourceOpenStreamOpVTable) }
    }

    async fn wait_ready_async(
        &self,
        plugin_free: PluginFreeFn,
        timeout: Duration,
        timeout_message: &'static str,
    ) -> Result<StAsyncOpState> {
        AbiOpWaitFuture::new(
            self.handle_addr,
            self.vtable().poll,
            self.vtable().cancel,
            self.vtable().set_notifier,
            plugin_free,
            timeout,
            timeout_message,
        )
        .await
    }

    fn take_stream(
        &mut self,
        plugin_free: PluginFreeFn,
    ) -> Result<(*const StIoVTable, *mut c_void, StStr)> {
        let mut out_io_vtable: *const StIoVTable = core::ptr::null();
        let mut out_io_handle: *mut c_void = core::ptr::null_mut();
        let mut out_meta = StStr::empty();
        let status = (self.vtable().take_stream)(
            self.handle_ptr(),
            &mut out_io_vtable,
            &mut out_io_handle,
            &mut out_meta,
        );
        status_to_result("Source open_stream take_stream", status, plugin_free)?;
        Ok((out_io_vtable, out_io_handle, out_meta))
    }
}

impl Drop for SourceOpenStreamOpOwned {
    fn drop(&mut self) {
        if self.handle_addr != 0 && self.vtable_addr != 0 {
            (self.vtable().destroy)(self.handle_ptr());
            self.handle_addr = 0;
        }
    }
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

    pub fn instance_id(&self) -> crate::runtime::instance_registry::InstanceId {
        self.ctx.instance_id
    }

    fn begin_list_items_op(&mut self, request_json: &str) -> Result<SourceListItemsOpOwned> {
        let mut raw = StSourceListItemsOpRef {
            handle: core::ptr::null_mut(),
            vtable: core::ptr::null(),
            reserved0: 0,
            reserved1: 0,
        };
        let status = unsafe {
            ((*self.vtable).begin_list_items_json_utf8)(
                self.handle,
                ststr_from_str(request_json),
                &mut raw,
            )
        };
        status_to_result("Source begin_list_items_json", status, self.ctx.plugin_free)?;
        if raw.handle.is_null() || raw.vtable.is_null() {
            return Err(anyhow!(
                "source begin_list_items_json returned null op handle/vtable"
            ));
        }
        Ok(SourceListItemsOpOwned::from_raw(raw))
    }

    pub async fn list_items_json(&mut self, request_json: &str) -> Result<String> {
        let mut op = self.begin_list_items_op(request_json)?;
        let state = op
            .wait_ready_async(
                self.ctx.plugin_free,
                SOURCE_LIST_TIMEOUT,
                "source list_items operation timed out",
            )
            .await?;

        match state {
            StAsyncOpState::Ready => op.take_json_utf8(self.ctx.plugin_free),
            StAsyncOpState::Cancelled => Err(anyhow!("source list_items operation cancelled")),
            StAsyncOpState::Failed => {
                let _ = op.take_json_utf8(self.ctx.plugin_free);
                Err(anyhow!("source list_items operation failed"))
            },
            StAsyncOpState::Pending => Err(anyhow!("source list_items operation still pending")),
        }
    }

    fn begin_open_stream_op(&mut self, track_json: &str) -> Result<SourceOpenStreamOpOwned> {
        let mut raw = StSourceOpenStreamOpRef {
            handle: core::ptr::null_mut(),
            vtable: core::ptr::null(),
            reserved0: 0,
            reserved1: 0,
        };
        let status = unsafe {
            ((*self.vtable).begin_open_stream)(self.handle, ststr_from_str(track_json), &mut raw)
        };
        status_to_result("Source begin_open_stream", status, self.ctx.plugin_free)?;
        if raw.handle.is_null() || raw.vtable.is_null() {
            return Err(anyhow!(
                "source begin_open_stream returned null op handle/vtable"
            ));
        }
        Ok(SourceOpenStreamOpOwned::from_raw(raw))
    }

    pub async fn open_stream(
        &mut self,
        track_json: &str,
    ) -> Result<(SourceOpenStreamResult, Option<String>)> {
        let mut op = self.begin_open_stream_op(track_json)?;
        let state = op
            .wait_ready_async(
                self.ctx.plugin_free,
                SOURCE_OPEN_TIMEOUT,
                "source open_stream operation timed out",
            )
            .await?;

        match state {
            StAsyncOpState::Ready => {
                let (out_io_vtable, out_io_handle, out_meta) =
                    op.take_stream(self.ctx.plugin_free)?;
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
            },
            StAsyncOpState::Cancelled => Err(anyhow!("source open_stream operation cancelled")),
            StAsyncOpState::Failed => {
                if let Ok((_out_io_vtable, out_io_handle, out_meta)) =
                    op.take_stream(self.ctx.plugin_free)
                {
                    if !out_meta.ptr.is_null() && out_meta.len != 0 {
                        let _ = take_plugin_string(out_meta, self.ctx.plugin_free);
                    }
                    if !out_io_handle.is_null() {
                        self.close_stream(out_io_handle);
                    }
                }
                Err(anyhow!("source open_stream operation failed"))
            },
            StAsyncOpState::Pending => Err(anyhow!("source open_stream operation still pending")),
        }
    }

    pub fn close_stream(&mut self, io_handle: *mut c_void) {
        if io_handle.is_null() {
            return;
        }
        let mut op = StUnitOpRef {
            handle: core::ptr::null_mut(),
            vtable: core::ptr::null(),
            reserved0: 0,
            reserved1: 0,
        };
        let status =
            unsafe { ((*self.vtable).begin_close_stream)(self.handle, io_handle, &mut op) };
        if status.code != 0 || op.handle.is_null() || op.vtable.is_null() {
            return;
        }
        if let Ok(state) =
            wait_unit_op_state(&op, self.ctx.plugin_free, "Source close_stream op wait")
            && state != StAsyncOpState::Pending
        {
            let _ = status_to_result(
                "Source close_stream finish",
                unsafe { ((*op.vtable).finish)(op.handle) },
                self.ctx.plugin_free,
            );
        }
        unsafe { ((*op.vtable).destroy)(op.handle) };
    }

    pub fn plan_config_update_json(&self, new_config_json: &str) -> Result<ConfigUpdatePlan> {
        let Some(plan_fn) = (unsafe { (*self.vtable).begin_plan_config_update_json_utf8 }) else {
            return Ok(ConfigUpdatePlan {
                mode: StConfigUpdateMode::Recreate,
                reason: Some("plugin does not implement plan_config_update".to_string()),
            });
        };

        let mut op = StConfigUpdatePlanOpRef {
            handle: core::ptr::null_mut(),
            vtable: core::ptr::null(),
            reserved0: 0,
            reserved1: 0,
        };
        let status = (plan_fn)(self.handle, ststr_from_str(new_config_json), &mut op);
        status_to_result(
            "Source begin_plan_config_update_json",
            status,
            self.ctx.plugin_free,
        )?;
        if op.handle.is_null() || op.vtable.is_null() {
            return Err(anyhow!(
                "source begin_plan_config_update_json returned null op handle/vtable"
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
                    "Source plan_config_update_json",
                    status,
                    self.ctx.plugin_free,
                )?;
                Ok(plan_from_ffi(out, self.ctx.plugin_free))
            },
            StAsyncOpState::Cancelled => {
                Err(anyhow!("source plan_config_update operation cancelled"))
            },
            StAsyncOpState::Failed => {
                let mut out = StConfigUpdatePlan {
                    mode: StConfigUpdateMode::Reject,
                    reason_utf8: StStr::empty(),
                };
                let status = unsafe { ((*op.vtable).take_plan)(op.handle, &mut out) };
                status_to_result(
                    "Source plan_config_update op failed",
                    status,
                    self.ctx.plugin_free,
                )?;
                Err(anyhow!("source plan_config_update operation failed"))
            },
            StAsyncOpState::Pending => {
                Err(anyhow!("source plan_config_update operation still pending"))
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
                    let msg = "source apply_config_update not supported".to_string();
                    let _ = self.ctx.updates.finish_failed(&req, msg.clone());
                    return Err(anyhow!(msg));
                };

                let mut op = StUnitOpRef {
                    handle: core::ptr::null_mut(),
                    vtable: core::ptr::null(),
                    reserved0: 0,
                    reserved1: 0,
                };
                let status = (apply_fn)(self.handle, ststr_from_str(&req.config_json), &mut op);
                let apply_res = (|| {
                    status_to_result(
                        "Source begin_apply_config_update_json",
                        status,
                        self.ctx.plugin_free,
                    )?;
                    if op.handle.is_null() || op.vtable.is_null() {
                        return Err(anyhow!(
                            "source begin_apply_config_update returned null op handle/vtable"
                        ));
                    }
                    match wait_unit_op_state(
                        &op,
                        self.ctx.plugin_free,
                        "Source apply_config_update op wait",
                    )? {
                        StAsyncOpState::Ready => {
                            let status = unsafe { ((*op.vtable).finish)(op.handle) };
                            status_to_result(
                                "Source apply_config_update_json",
                                status,
                                self.ctx.plugin_free,
                            )
                        },
                        StAsyncOpState::Cancelled => {
                            Err(anyhow!("source apply_config_update operation cancelled"))
                        },
                        StAsyncOpState::Failed => {
                            let status = unsafe { ((*op.vtable).finish)(op.handle) };
                            status_to_result(
                                "Source apply_config_update op failed",
                                status,
                                self.ctx.plugin_free,
                            )?;
                            Err(anyhow!("source apply_config_update operation failed"))
                        },
                        StAsyncOpState::Pending => Err(anyhow!(
                            "source apply_config_update operation still pending"
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
                    .unwrap_or_else(|| "source rejected config update".to_string());
                Ok(self.ctx.updates.finish_rejected(&req, reason))
            },
        }
    }

    pub fn export_state_json(&self) -> Result<Option<String>> {
        let Some(export_fn) = (unsafe { (*self.vtable).begin_export_state_json_utf8 }) else {
            return Ok(None);
        };

        let mut op = StJsonOpRef {
            handle: core::ptr::null_mut(),
            vtable: core::ptr::null(),
            reserved0: 0,
            reserved1: 0,
        };
        let status = (export_fn)(self.handle, &mut op);
        status_to_result(
            "Source begin_export_state_json",
            status,
            self.ctx.plugin_free,
        )?;
        if op.handle.is_null() || op.vtable.is_null() {
            return Err(anyhow!(
                "source begin_export_state_json returned null op handle/vtable"
            ));
        }

        let result = (|| match wait_json_op_state(&op, self.ctx.plugin_free)? {
            StAsyncOpState::Ready => {
                let mut out = StStr::empty();
                let status = unsafe { ((*op.vtable).take_json_utf8)(op.handle, &mut out) };
                status_to_result("Source export_state_json", status, self.ctx.plugin_free)?;
                let raw = take_plugin_string(out, self.ctx.plugin_free);
                if raw.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(raw))
                }
            },
            StAsyncOpState::Cancelled => Err(anyhow!("source export_state operation cancelled")),
            StAsyncOpState::Failed => {
                let mut out = StStr::empty();
                let status = unsafe { ((*op.vtable).take_json_utf8)(op.handle, &mut out) };
                status_to_result(
                    "Source export_state op failed",
                    status,
                    self.ctx.plugin_free,
                )?;
                Err(anyhow!("source export_state operation failed"))
            },
            StAsyncOpState::Pending => Err(anyhow!("source export_state operation still pending")),
        })();

        unsafe { ((*op.vtable).destroy)(op.handle) };
        result
    }

    pub fn import_state_json(&mut self, state_json: &str) -> Result<()> {
        let Some(import_fn) = (unsafe { (*self.vtable).begin_import_state_json_utf8 }) else {
            return Err(anyhow!("source import_state_json not supported"));
        };

        let mut op = StUnitOpRef {
            handle: core::ptr::null_mut(),
            vtable: core::ptr::null(),
            reserved0: 0,
            reserved1: 0,
        };
        let status = (import_fn)(self.handle, ststr_from_str(state_json), &mut op);
        status_to_result(
            "Source begin_import_state_json",
            status,
            self.ctx.plugin_free,
        )?;
        if op.handle.is_null() || op.vtable.is_null() {
            return Err(anyhow!(
                "source begin_import_state_json returned null op handle/vtable"
            ));
        }

        let result =
            (|| match wait_unit_op_state(&op, self.ctx.plugin_free, "Source import_state op wait")?
            {
                StAsyncOpState::Ready => {
                    let status = unsafe { ((*op.vtable).finish)(op.handle) };
                    status_to_result("Source import_state_json", status, self.ctx.plugin_free)
                },
                StAsyncOpState::Cancelled => {
                    Err(anyhow!("source import_state operation cancelled"))
                },
                StAsyncOpState::Failed => {
                    let status = unsafe { ((*op.vtable).finish)(op.handle) };
                    status_to_result(
                        "Source import_state op failed",
                        status,
                        self.ctx.plugin_free,
                    )?;
                    Err(anyhow!("source import_state operation failed"))
                },
                StAsyncOpState::Pending => {
                    Err(anyhow!("source import_state operation still pending"))
                },
            })();

        unsafe { ((*op.vtable).destroy)(op.handle) };
        result
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
        let status = unsafe { ((*op.vtable).wait)(op.handle, SOURCE_OP_WAIT_SLICE_MS, &mut state) };
        status_to_result(what, status, plugin_free)?;
        if state != StAsyncOpState::Pending {
            return Ok(state);
        }
        if started.elapsed() >= SOURCE_UNIT_TIMEOUT {
            let _ = status_to_result(
                "Source unit op cancel",
                unsafe { ((*op.vtable).cancel)(op.handle) },
                plugin_free,
            );
            return Err(anyhow!(
                "source unit operation timed out after {}ms",
                SOURCE_UNIT_TIMEOUT.as_millis()
            ));
        }
    }
}

fn wait_json_op_state(op: &StJsonOpRef, plugin_free: PluginFreeFn) -> Result<StAsyncOpState> {
    let started = Instant::now();
    loop {
        let mut state = StAsyncOpState::Pending;
        let status = unsafe { ((*op.vtable).wait)(op.handle, SOURCE_OP_WAIT_SLICE_MS, &mut state) };
        status_to_result("Source json op wait", status, plugin_free)?;
        if state != StAsyncOpState::Pending {
            return Ok(state);
        }
        if started.elapsed() >= SOURCE_JSON_TIMEOUT {
            let _ = status_to_result(
                "Source json op cancel",
                unsafe { ((*op.vtable).cancel)(op.handle) },
                plugin_free,
            );
            return Err(anyhow!(
                "source json operation timed out after {}ms",
                SOURCE_JSON_TIMEOUT.as_millis()
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
        let status = unsafe { ((*op.vtable).wait)(op.handle, SOURCE_OP_WAIT_SLICE_MS, &mut state) };
        status_to_result("Source plan op wait", status, plugin_free)?;
        if state != StAsyncOpState::Pending {
            return Ok(state);
        }
        if started.elapsed() >= SOURCE_PLAN_TIMEOUT {
            let _ = status_to_result(
                "Source plan op cancel",
                unsafe { ((*op.vtable).cancel)(op.handle) },
                plugin_free,
            );
            return Err(anyhow!(
                "source plan operation timed out after {}ms",
                SOURCE_PLAN_TIMEOUT.as_millis()
            ));
        }
    }
}

impl Drop for SourceCatalogInstance {
    fn drop(&mut self) {
        if !self.handle.is_null() && !self.vtable.is_null() {
            unsafe { ((*self.vtable).destroy)(self.handle) };
            self.handle = core::ptr::null_mut();
        }
    }
}

// SAFETY: The worker model requires moving instances across thread boundaries and
// using each instance from exactly one worker thread at a time.
unsafe impl Send for SourceCatalogInstance {}
