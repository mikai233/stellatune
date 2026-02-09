use stellatune_plugin_api::{StConfigUpdateMode, StConfigUpdatePlan};

use crate::{SdkError, SdkResult, StStatus, StStr, alloc_utf8_bytes};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdatePlan {
    HotApply,
    Recreate,
    Reject { reason: String },
}

impl UpdatePlan {
    pub fn hot_apply() -> Self {
        Self::HotApply
    }

    pub fn recreate() -> Self {
        Self::Recreate
    }

    pub fn reject(reason: impl Into<String>) -> Self {
        Self::Reject {
            reason: reason.into(),
        }
    }
}

pub trait ConfigUpdatable {
    /// Decide whether the incoming config can be hot-applied, requires recreate, or should be rejected.
    fn plan_config_update_json(&self, _new_config_json: &str) -> SdkResult<UpdatePlan> {
        Ok(UpdatePlan::Recreate)
    }

    /// Apply a hot config update in-place.
    fn apply_config_update_json(&mut self, _new_config_json: &str) -> SdkResult<()> {
        Err(SdkError::msg("hot config update unsupported"))
    }

    /// Export migratable state used by recreate flow. `None` means no state transfer.
    fn export_state_json(&self) -> SdkResult<Option<String>> {
        Ok(None)
    }

    /// Import state exported from previous instance. Default ignores incoming state.
    fn import_state_json(&mut self, _state_json: &str) -> SdkResult<()> {
        Ok(())
    }
}

/// Convert a high-level plan into FFI plan struct.
///
/// If `Reject` contains a reason, this function allocates plugin-owned UTF-8 bytes.
/// Host must free via `plugin_free` from module vtable.
pub fn plan_to_ffi(plan: UpdatePlan) -> StConfigUpdatePlan {
    match plan {
        UpdatePlan::HotApply => StConfigUpdatePlan {
            mode: StConfigUpdateMode::HotApply,
            reason_utf8: StStr::empty(),
        },
        UpdatePlan::Recreate => StConfigUpdatePlan {
            mode: StConfigUpdateMode::Recreate,
            reason_utf8: StStr::empty(),
        },
        UpdatePlan::Reject { reason } => StConfigUpdatePlan {
            mode: StConfigUpdateMode::Reject,
            reason_utf8: alloc_utf8_bytes(&reason),
        },
    }
}

pub fn write_plan_to_ffi(out: *mut StConfigUpdatePlan, plan: UpdatePlan) -> SdkResult<()> {
    if out.is_null() {
        return Err(SdkError::invalid_arg("null out plan pointer"));
    }
    let ffi = plan_to_ffi(plan);
    // Safety: caller passed non-null out pointer.
    unsafe {
        *out = ffi;
    }
    Ok(())
}

pub fn mode_to_status(plan: &UpdatePlan) -> StStatus {
    match plan {
        UpdatePlan::HotApply | UpdatePlan::Recreate => StStatus::ok(),
        UpdatePlan::Reject { .. } => StStatus::ok(),
    }
}
