use std::any::Any;
use std::sync::Arc;

use crossbeam_channel::Sender;
use stellatune_audio_core::pipeline::stages::{StageDispatchResult, StageTarget};

use crate::error::DecodeError;
use crate::workers::decode::state::DecodeWorkerState;

pub(crate) fn handle(
    target: StageTarget,
    control: Arc<dyn Any + Send + Sync>,
    resp_tx: Sender<Result<(), DecodeError>>,
    state: &mut DecodeWorkerState,
) -> bool {
    let result = (|| {
        if let Some(runner) = state.runner.as_mut() {
            let handled = runner.apply_stage_control_to(
                &target,
                Arc::clone(&control),
                Some(&state.sink_session),
                &mut state.ctx,
            )?;
            if handled == StageDispatchResult::StageNotFound {
                return Err(DecodeError::StageTargetNotFound {
                    target: target.clone(),
                });
            }
        }
        state.persisted_stage_controls.insert(target, control);
        Ok(())
    })();
    let _ = resp_tx.send(result);
    false
}
