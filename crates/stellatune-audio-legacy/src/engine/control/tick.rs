use std::time::Instant;

use tracing::debug;

use crate::output::output_spec_for_device;
use stellatune_runtime as global_runtime;

use super::{
    EngineState, InternalDispatchTx, internal_output_spec_failed_dispatch,
    internal_output_spec_ready_dispatch, output_spec_for_plugin_sink,
};

pub(super) fn ensure_output_spec_prewarm(
    state: &mut EngineState,
    internal_tx: &InternalDispatchTx,
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
                let _ = tx.send(internal_output_spec_ready_dispatch(
                    spec,
                    t0.elapsed().as_millis() as u64,
                    token,
                ));
            },
            Ok(Err(e)) => {
                let _ = tx.send(internal_output_spec_failed_dispatch(
                    e.to_string(),
                    t0.elapsed().as_millis() as u64,
                    token,
                ));
            },
            Err(join_err) => {
                let _ = tx.send(internal_output_spec_failed_dispatch(
                    format!("output spec prewarm task join failed: {join_err}"),
                    t0.elapsed().as_millis() as u64,
                    token,
                ));
            },
        }
    });
}

pub(super) fn output_backend_for_selected(
    backend: crate::types::AudioBackend,
) -> crate::output::AudioBackend {
    match backend {
        crate::types::AudioBackend::Shared => crate::output::AudioBackend::Shared,
        crate::types::AudioBackend::WasapiExclusive => crate::output::AudioBackend::WasapiExclusive,
    }
}
