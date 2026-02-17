use std::any::Any;

use crossbeam_channel::Sender;

use crate::workers::decode_worker::state::DecodeWorkerState;

pub(crate) fn handle(
    stage_key: String,
    control: Box<dyn Any + Send>,
    resp_tx: Sender<Result<(), String>>,
    state: &mut DecodeWorkerState,
) -> bool {
    let result = (|| {
        if let Some(runner) = state.runner.as_mut() {
            let handled = runner
                .apply_transform_control_to(&stage_key, control.as_ref(), &mut state.ctx)
                .map_err(|e| e.to_string())?;
            if !handled {
                return Err(format!(
                    "transform stage not found for stage key: {stage_key}"
                ));
            }
        }
        state.persisted_stage_controls.insert(stage_key, control);
        Ok(())
    })();
    let _ = resp_tx.send(result);
    false
}
