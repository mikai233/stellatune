use std::sync::Arc;

use crossbeam_channel::Sender;
use stellatune_audio_core::pipeline::stages::{
    StageRuntimeUpdate, StageRuntimeUpdateDispatchResult, StageTarget,
};

use crate::error::DecodeError;
use crate::workers::decode::state::DecodeWorkerState;

pub(crate) fn handle(
    target: StageTarget,
    update: Arc<dyn StageRuntimeUpdate>,
    resp_tx: Sender<Result<(), DecodeError>>,
    state: &mut DecodeWorkerState,
) -> bool {
    let result = (|| {
        if let Some(runner) = state.runner.as_mut() {
            let handled = runner.apply_stage_runtime_update_to(
                &target,
                Arc::clone(&update),
                Some(&state.sink_session),
                &mut state.ctx,
            )?;
            if handled == StageRuntimeUpdateDispatchResult::StageNotFound {
                return Err(DecodeError::StageRuntimeUpdateTargetNotFound {
                    target: target.clone(),
                });
            }
        }
        state.persisted_stage_runtime_updates.insert(target, update);
        Ok(())
    })();
    let _ = resp_tx.send(result);
    false
}
