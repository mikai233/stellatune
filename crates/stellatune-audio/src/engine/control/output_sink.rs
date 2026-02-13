use std::sync::Arc;

use crossbeam_channel::Sender;
use tracing::warn;

use stellatune_output::OutputSpec;
use stellatune_plugin_api::{StAudioSpec, StOutputSinkNegotiatedSpec};
use stellatune_plugins::runtime::worker_controller::WorkerApplyPendingOutcome;

use crate::engine::session::OutputSinkWorkerStartArgs;

use super::{
    CachedOutputSinkInstance, DecodeCtrl, EngineState, InternalMsg, OUTPUT_SINK_QUEUE_CAP_MESSAGES,
    OpenOutputSinkWorkerArgs, OutputSinkNegotiationCache, OutputSinkWorker, OutputSinkWorkerSpec,
    PLUGIN_SINK_DEFAULT_CHUNK_FRAMES, PLUGIN_SINK_FALLBACK_CHANNELS,
    PLUGIN_SINK_FALLBACK_SAMPLE_RATE, PLUGIN_SINK_MIN_HIGH_WATERMARK_MS,
    PLUGIN_SINK_MIN_LOW_WATERMARK_MS, RuntimeInstanceSlotKey, debug_metrics, with_runtime_service,
};
use crate::engine::control::runtime_query::apply_or_recreate_output_sink_instance;

fn create_output_sink_cached_instance(
    service: &stellatune_plugins::runtime::handle::SharedPluginRuntimeService,
    plugin_id: &str,
    type_id: &str,
    config_json: &str,
) -> Result<CachedOutputSinkInstance, String> {
    let endpoint =
        stellatune_runtime::block_on(service.bind_output_sink_worker_endpoint(plugin_id, type_id))
            .map_err(|e| e.to_string())?;
    let (mut controller, control_rx) = endpoint.into_controller(config_json.to_string());
    match controller.apply_pending().map_err(|e| e.to_string())? {
        WorkerApplyPendingOutcome::Created | WorkerApplyPendingOutcome::Recreated => {
            Ok(CachedOutputSinkInstance {
                config_json: config_json.to_string(),
                controller,
                control_rx,
            })
        }
        WorkerApplyPendingOutcome::Destroyed | WorkerApplyPendingOutcome::Idle => Err(format!(
            "output sink controller did not create instance for {plugin_id}::{type_id}"
        )),
    }
}

pub(super) fn output_spec_for_plugin_sink(state: &EngineState) -> OutputSpec {
    let start_at_ms = state.position_ms.max(0) as u64;
    if let (Some(path), Some(worker)) =
        (state.current_track.as_deref(), state.decode_worker.as_ref())
        && let Some(track_info) = worker.peek_promoted_track_info(path, start_at_ms)
    {
        return OutputSpec {
            sample_rate: track_info.sample_rate.max(1),
            channels: track_info.channels.max(1),
        };
    }

    OutputSpec {
        sample_rate: PLUGIN_SINK_FALLBACK_SAMPLE_RATE,
        channels: PLUGIN_SINK_FALLBACK_CHANNELS,
    }
}

pub(super) fn output_sink_queue_watermarks_ms(sample_rate: u32, chunk_frames: u32) -> (i64, i64) {
    let frames_per_chunk = if chunk_frames == 0 {
        PLUGIN_SINK_DEFAULT_CHUNK_FRAMES as u64
    } else {
        chunk_frames as u64
    };
    let capacity_frames = frames_per_chunk
        .saturating_mul(OUTPUT_SINK_QUEUE_CAP_MESSAGES as u64)
        .max(1);
    let sample_rate = sample_rate.max(1) as u64;
    let capacity_ms = ((capacity_frames.saturating_mul(1000)) / sample_rate)
        .max(PLUGIN_SINK_MIN_LOW_WATERMARK_MS as u64);
    let high = ((capacity_ms.saturating_mul(3)) / 4)
        .max(PLUGIN_SINK_MIN_HIGH_WATERMARK_MS as u64)
        .min(capacity_ms) as i64;
    let low = (high / 2).max(PLUGIN_SINK_MIN_LOW_WATERMARK_MS);
    (low.min(high.saturating_sub(1)), high)
}

pub(super) fn negotiate_output_sink_spec(
    state: &mut EngineState,
    desired_spec: OutputSpec,
) -> Result<StOutputSinkNegotiatedSpec, String> {
    let route = state
        .desired_output_sink_route
        .clone()
        .ok_or_else(|| "output sink route not configured".to_string())?;
    let config_json = serde_json::to_string(&route.config)
        .map_err(|e| format!("invalid output sink config json: {e}"))?;
    let target_json = serde_json::to_string(&route.target)
        .map_err(|e| format!("invalid output sink target json: {e}"))?;

    let key = RuntimeInstanceSlotKey::new(&route.plugin_id, &route.type_id);
    if !state.output_sink_instances.contains_key(&key) {
        let created = with_runtime_service(|service| {
            create_output_sink_cached_instance(
                service,
                &route.plugin_id,
                &route.type_id,
                &config_json,
            )
            .map_err(|e| format!("output sink create failed: {e}"))
        })?;
        state.output_sink_instances.insert(key.clone(), created);
    }

    let entry = state
        .output_sink_instances
        .get_mut(&key)
        .ok_or_else(|| "output sink instance cache insertion failed".to_string())?;
    apply_or_recreate_output_sink_instance(&route.plugin_id, &route.type_id, entry, &config_json)?;
    let instance = entry.controller.instance_mut().ok_or_else(|| {
        format!(
            "output sink instance unavailable for {}::{}",
            route.plugin_id, route.type_id
        )
    })?;
    instance
        .negotiate_spec(
            &target_json,
            StAudioSpec {
                sample_rate: desired_spec.sample_rate.max(1),
                channels: desired_spec.channels.max(1),
                reserved: 0,
            },
        )
        .map_err(|e| format!("output sink negotiate failed: {e}"))
}

pub(super) fn resolve_output_spec_and_sink_chunk(
    state: &mut EngineState,
    non_sink_out_spec: OutputSpec,
) -> Result<OutputSpec, String> {
    if state.desired_output_sink_route.is_none() {
        state.output_sink_chunk_frames = 0;
        state.output_sink_negotiation_cache = None;
        return Ok(non_sink_out_spec);
    }

    state.output_sink_chunk_frames = 0;
    let desired_spec = output_spec_for_plugin_sink(state);
    if let (Some(route), Some(cached)) = (
        state.desired_output_sink_route.as_ref(),
        state.output_sink_negotiation_cache.as_ref(),
    ) && cached.route == *route
        && cached.desired_spec == desired_spec
    {
        state.output_sink_chunk_frames = cached.negotiated.preferred_chunk_frames;
        return Ok(OutputSpec {
            sample_rate: cached.negotiated.spec.sample_rate.max(1),
            channels: cached.negotiated.spec.channels.max(1),
        });
    }

    let negotiated = negotiate_output_sink_spec(state, desired_spec)?;
    let route = state
        .desired_output_sink_route
        .clone()
        .ok_or_else(|| "output sink route not configured".to_string())?;
    state.output_sink_negotiation_cache = Some(OutputSinkNegotiationCache {
        route,
        desired_spec,
        negotiated,
    });
    state.output_sink_chunk_frames = negotiated.preferred_chunk_frames;
    Ok(OutputSpec {
        sample_rate: negotiated.spec.sample_rate.max(1),
        channels: negotiated.spec.channels.max(1),
    })
}

pub(super) fn open_output_sink_worker(
    args: OpenOutputSinkWorkerArgs<'_>,
) -> Result<OutputSinkWorker, String> {
    let config_json = serde_json::to_string(&args.route.config)
        .map_err(|e| format!("invalid output sink config json: {e}"))?;
    let target_json = serde_json::to_string(&args.route.target)
        .map_err(|e| format!("invalid output sink target json: {e}"))?;
    OutputSinkWorker::start(OutputSinkWorkerStartArgs {
        plugin_id: args.route.plugin_id.clone(),
        type_id: args.route.type_id.clone(),
        target_json,
        config_json,
        channels: args.channels,
        sample_rate: args.sample_rate,
        volume: args.volume,
        transition_gain: args.transition_gain,
        transition_target_gain: args.transition_target_gain,
        transition_ramp_ms: args.transition_ramp_ms,
        internal_tx: args.internal_tx.clone(),
    })
}

pub(super) fn sync_output_sink_with_active_session(
    state: &mut EngineState,
    internal_tx: &Sender<InternalMsg>,
) -> Result<(), String> {
    let Some(session) = state.session.as_ref() else {
        shutdown_output_sink_worker(state);
        return Ok(());
    };
    let ctrl_tx = session.ctrl_tx.clone();
    let sample_rate = session.out_sample_rate;
    let channels = session.out_channels;
    let transition_gain = Arc::clone(&session.transition_gain);
    let transition_target_gain = Arc::clone(&session.transition_target_gain);
    let transition_ramp_ms = Arc::clone(&session.transition_ramp_ms);
    let desired_route = state.desired_output_sink_route.clone();
    let Some(route) = desired_route else {
        let _ = ctrl_tx.send(DecodeCtrl::SetOutputSinkTx {
            tx: None,
            output_sink_chunk_frames: 0,
        });
        shutdown_output_sink_worker(state);
        return Ok(());
    };
    debug_metrics::note_output_sink_sample_rate(sample_rate);
    let had_active_worker = state.output_sink_worker.is_some();
    let desired_spec = OutputSinkWorkerSpec {
        route: route.clone(),
        sample_rate,
        channels,
        chunk_frames: state.output_sink_chunk_frames,
    };
    if let (Some(worker), Some(active_spec)) = (
        state.output_sink_worker.as_ref(),
        state.output_sink_worker_spec.as_ref(),
    ) {
        if active_spec == &desired_spec {
            let _ = ctrl_tx.send(DecodeCtrl::SetOutputSinkTx {
                tx: Some(worker.sender()),
                output_sink_chunk_frames: state.output_sink_chunk_frames,
            });
            return Ok(());
        }

        let same_runtime_identity = active_spec.route.plugin_id == desired_spec.route.plugin_id
            && active_spec.route.type_id == desired_spec.route.type_id
            && active_spec.route.target == desired_spec.route.target
            && active_spec.sample_rate == desired_spec.sample_rate
            && active_spec.channels == desired_spec.channels;
        if same_runtime_identity {
            let config_json = serde_json::to_string(&desired_spec.route.config)
                .map_err(|e| format!("invalid output sink config json: {e}"))?;
            match worker.apply_config_json(config_json) {
                Ok(()) => {
                    state.output_sink_worker_spec = Some(desired_spec);
                    let _ = ctrl_tx.send(DecodeCtrl::SetOutputSinkTx {
                        tx: Some(worker.sender()),
                        output_sink_chunk_frames: state.output_sink_chunk_frames,
                    });
                    return Ok(());
                }
                Err(e) => {
                    warn!("output sink hot config update failed, fallback to recreate: {e}");
                }
            }
        }
    }

    let _ = ctrl_tx.send(DecodeCtrl::SetOutputSinkTx {
        tx: None,
        output_sink_chunk_frames: 0,
    });
    shutdown_output_sink_worker(state);
    if had_active_worker {
        debug_metrics::note_output_sink_recreate();
    }

    let worker = open_output_sink_worker(OpenOutputSinkWorkerArgs {
        route: &route,
        sample_rate,
        channels,
        volume: Arc::clone(&state.volume_atomic),
        transition_gain,
        transition_target_gain,
        transition_ramp_ms,
        internal_tx,
    })?;
    let tx = worker.sender();
    state.output_sink_worker = Some(worker);
    state.output_sink_worker_spec = Some(desired_spec);
    let _ = ctrl_tx.send(DecodeCtrl::SetOutputSinkTx {
        tx: Some(tx),
        output_sink_chunk_frames: state.output_sink_chunk_frames,
    });
    Ok(())
}

pub(super) fn shutdown_output_sink_worker(state: &mut EngineState) {
    state.output_sink_worker_spec = None;
    let Some(worker) = state.output_sink_worker.take() else {
        return;
    };
    // Track switching should not block on sink drain/flush.
    worker.shutdown(false);
}
