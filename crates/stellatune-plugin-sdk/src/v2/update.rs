use stellatune_plugin_api::v2::{StConfigUpdateModeV2, StConfigUpdatePlanV2};

use crate::{SdkError, SdkResult, StStatus, StStr, alloc_utf8_bytes};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdatePlanV2 {
    HotApply,
    Recreate,
    Reject { reason: String },
}

impl UpdatePlanV2 {
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

pub trait ConfigUpdatableV2 {
    /// Decide whether the incoming config can be hot-applied, requires recreate, or should be rejected.
    fn plan_config_update_json(&self, _new_config_json: &str) -> SdkResult<UpdatePlanV2> {
        Ok(UpdatePlanV2::Recreate)
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
pub fn plan_to_ffi(plan: UpdatePlanV2) -> StConfigUpdatePlanV2 {
    match plan {
        UpdatePlanV2::HotApply => StConfigUpdatePlanV2 {
            mode: StConfigUpdateModeV2::HotApply,
            reason_utf8: StStr::empty(),
        },
        UpdatePlanV2::Recreate => StConfigUpdatePlanV2 {
            mode: StConfigUpdateModeV2::Recreate,
            reason_utf8: StStr::empty(),
        },
        UpdatePlanV2::Reject { reason } => StConfigUpdatePlanV2 {
            mode: StConfigUpdateModeV2::Reject,
            reason_utf8: alloc_utf8_bytes(&reason),
        },
    }
}

pub fn write_plan_to_ffi(out: *mut StConfigUpdatePlanV2, plan: UpdatePlanV2) -> SdkResult<()> {
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

pub fn mode_to_status(plan: &UpdatePlanV2) -> StStatus {
    match plan {
        UpdatePlanV2::HotApply | UpdatePlanV2::Recreate => StStatus::ok(),
        UpdatePlanV2::Reject { .. } => StStatus::ok(),
    }
}
