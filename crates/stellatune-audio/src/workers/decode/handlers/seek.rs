use crossbeam_channel::Sender;

use crate::config::engine::PlayerState;
use crate::workers::decode::handlers::gain_transition;
use crate::workers::decode::state::DecodeWorkerState;
use crate::workers::decode::{DecodeWorkerEvent, DecodeWorkerEventCallback};

pub(crate) fn handle(
    position_ms: i64,
    resp_tx: Sender<Result<(), String>>,
    callback: &DecodeWorkerEventCallback,
    state: &mut DecodeWorkerState,
) -> bool {
    let transition = state.gain_transition;
    let result = if let Some(active_runner) = state.runner.as_mut() {
        let was_playing = state.state == PlayerState::Playing;
        if was_playing {
            let available_frames_hint = active_runner.playable_remaining_frames_hint();
            let _ = gain_transition::run_interrupt_fade_out(
                active_runner,
                &mut state.sink_session,
                &mut state.ctx,
                transition,
                transition.seek_fade_out_ms,
                available_frames_hint,
            );
        }
        match active_runner
            .seek(position_ms, &mut state.sink_session, &mut state.ctx)
            .map_err(|e| e.to_string())
        {
            Ok(()) => {
                if was_playing {
                    if let Err(error) = gain_transition::request_fade_in_with_runner(
                        active_runner,
                        &mut state.ctx,
                        transition,
                        transition.seek_fade_in_ms,
                    ) {
                        Err(error.to_string())
                    } else {
                        let position_ms = position_ms.max(0);
                        state.ctx.position_ms = position_ms;
                        callback(DecodeWorkerEvent::Position { position_ms });
                        Ok(())
                    }
                } else {
                    let position_ms = position_ms.max(0);
                    state.ctx.position_ms = position_ms;
                    callback(DecodeWorkerEvent::Position { position_ms });
                    Ok(())
                }
            },
            Err(error) => Err(error),
        }
    } else {
        Err("no active pipeline to seek".to_string())
    };
    let _ = resp_tx.send(result);
    false
}
