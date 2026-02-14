use std::time::Instant;

use crossbeam_channel::Sender;
use tracing::debug;

use stellatune_output::output_spec_for_device;
use stellatune_runtime as global_runtime;

use super::{EngineState, InternalMsg, output_spec_for_plugin_sink};

pub(super) fn ensure_output_spec_prewarm(
    state: &mut EngineState,
    internal_tx: &Sender<InternalMsg>,
) {
    if state.cached_output_spec.is_some() || state.output_spec_prewarm_inflight {
        return;
    }

    if state.desired_output_sink_route.is_some() {
        let spec = output_spec_for_plugin_sink(state);
        state.cached_output_spec = Some(spec);
        state.output_spec_prewarm_inflight = false;
        debug!(
            "output_spec prewarm bypassed for plugin sink: {}Hz {}ch",
            spec.sample_rate, spec.channels
        );
        return;
    }

    state.output_spec_prewarm_inflight = true;
    let token = state.output_spec_token;
    let backend = output_backend_for_selected(state.selected_backend);
    let device_id = state.selected_device_id.clone();
    let tx = internal_tx.clone();
    global_runtime::spawn(async move {
        let t0 = Instant::now();
        let result =
            tokio::task::spawn_blocking(move || output_spec_for_device(backend, device_id)).await;
        match result {
            Ok(Ok(spec)) => {
                let _ = tx.send(InternalMsg::OutputSpecReady {
                    spec,
                    took_ms: t0.elapsed().as_millis() as u64,
                    token,
                });
            }
            Ok(Err(e)) => {
                let _ = tx.send(InternalMsg::OutputSpecFailed {
                    message: e.to_string(),
                    took_ms: t0.elapsed().as_millis() as u64,
                    token,
                });
            }
            Err(join_err) => {
                let _ = tx.send(InternalMsg::OutputSpecFailed {
                    message: format!("output spec prewarm task join failed: {join_err}"),
                    took_ms: t0.elapsed().as_millis() as u64,
                    token,
                });
            }
        }
    });
}

pub(super) fn output_backend_for_selected(
    backend: stellatune_core::AudioBackend,
) -> stellatune_output::AudioBackend {
    match backend {
        stellatune_core::AudioBackend::Shared => stellatune_output::AudioBackend::Shared,
        stellatune_core::AudioBackend::WasapiExclusive => {
            stellatune_output::AudioBackend::WasapiExclusive
        }
    }
}
