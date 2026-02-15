use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender};
use tracing::{debug, warn};

use stellatune_plugin_api::StAudioSpec;
use stellatune_plugins::runtime::messages::WorkerControlMessage;
use stellatune_plugins::runtime::worker_controller::{
    WorkerApplyPendingOutcome, WorkerConfigUpdateOutcome,
};
use stellatune_plugins::runtime::worker_endpoint::OutputSinkWorkerController;

use crate::engine::config::{OUTPUT_SINK_WRITE_RETRY_SLEEP_MS, OUTPUT_SINK_WRITE_STALL_TIMEOUT_MS};
use crate::engine::control::{InternalDispatch, internal_output_error_dispatch};
use crate::engine::messages::{OutputSinkTx, OutputSinkWrite};
use crate::engine::update_events::emit_config_update_runtime_event;

use super::OUTPUT_SINK_QUEUE_CAP_MESSAGES;
pub(crate) struct OutputSinkWorker {
    tx: Sender<OutputSinkWrite>,
    ctrl_tx: Sender<OutputSinkControl>,
    pending_samples: Arc<AtomicUsize>,
    sink_runtime_queued_samples: Arc<AtomicUsize>,
    join: JoinHandle<()>,
}

pub(crate) struct OutputSinkWorkerStartArgs {
    pub(crate) plugin_id: String,
    pub(crate) type_id: String,
    pub(crate) target_json: String,
    pub(crate) config_json: String,
    pub(crate) channels: u16,
    pub(crate) sample_rate: u32,
    pub(crate) volume: Arc<AtomicU32>,
    pub(crate) transition_gain: Arc<AtomicU32>,
    pub(crate) transition_target_gain: Arc<AtomicU32>,
    pub(crate) transition_ramp_ms: Arc<AtomicU32>,
    pub(crate) internal_tx: Sender<InternalDispatch>,
}

enum OutputSinkControl {
    UpdateConfig {
        config_json: String,
        resp_tx: Sender<Result<(), String>>,
    },
    ResetForDisrupt {
        resp_tx: Sender<Result<(), String>>,
    },
}

pub(super) struct MasterGainProcessor {
    volume: Arc<AtomicU32>,
    transition_gain: Arc<AtomicU32>,
    transition_target_gain: Arc<AtomicU32>,
    transition_ramp_ms: Arc<AtomicU32>,
    transition_current: f32,
    transition_from: f32,
    transition_to: f32,
    transition_progress: f32,
    sample_rate: u32,
    last_ramp_ms: u32,
    last_target_gain: f32,
    transition_step: f32,
}

impl MasterGainProcessor {
    pub(super) fn new(
        volume: Arc<AtomicU32>,
        transition_gain: Arc<AtomicU32>,
        transition_target_gain: Arc<AtomicU32>,
        transition_ramp_ms: Arc<AtomicU32>,
        sample_rate: u32,
    ) -> Self {
        let transition_current =
            f32::from_bits(transition_gain.load(Ordering::Relaxed)).clamp(0.0, 1.0);
        let transition_target =
            f32::from_bits(transition_target_gain.load(Ordering::Relaxed)).clamp(0.0, 1.0);
        let ramp_ms = transition_ramp_ms.load(Ordering::Relaxed).max(1);
        let transition_step = calc_transition_step(sample_rate, ramp_ms);
        Self {
            volume,
            transition_gain,
            transition_target_gain,
            transition_ramp_ms,
            transition_current,
            transition_from: transition_current,
            transition_to: transition_target,
            transition_progress: if (transition_current - transition_target).abs() <= f32::EPSILON {
                1.0
            } else {
                0.0
            },
            sample_rate,
            last_ramp_ms: ramp_ms,
            last_target_gain: transition_target,
            transition_step,
        }
    }

    fn next_transition_gain(&mut self) -> f32 {
        let ramp_ms = self.transition_ramp_ms.load(Ordering::Relaxed).max(1);
        if ramp_ms != self.last_ramp_ms {
            self.last_ramp_ms = ramp_ms;
            self.transition_step = calc_transition_step(self.sample_rate, ramp_ms);
        }
        let target =
            f32::from_bits(self.transition_target_gain.load(Ordering::Relaxed)).clamp(0.0, 1.0);
        if (target - self.last_target_gain).abs() > f32::EPSILON {
            self.transition_from = self.transition_current;
            self.transition_to = target;
            self.transition_progress = 0.0;
            self.last_target_gain = target;
        }
        if self.transition_progress < 1.0 {
            self.transition_progress = (self.transition_progress + self.transition_step).min(1.0);
            self.transition_current = transition_gain_interpolate(
                self.transition_from,
                self.transition_to,
                self.transition_progress,
            );
        } else {
            self.transition_current = self.transition_to;
        }
        self.transition_gain
            .store(self.transition_current.to_bits(), Ordering::Relaxed);
        self.transition_current
    }

    pub(super) fn apply_sample(&mut self, sample: f32) -> f32 {
        let volume = f32::from_bits(self.volume.load(Ordering::Relaxed));
        let transition = self.next_transition_gain();
        sample * volume * transition
    }

    fn apply_in_place(&mut self, samples: &mut [f32]) {
        for sample in samples {
            *sample = self.apply_sample(*sample);
        }
    }
}

fn calc_transition_step(sample_rate: u32, ramp_ms: u32) -> f32 {
    let sample_rate = sample_rate.max(1) as f32;
    let ramp_ms = ramp_ms.max(1) as f32;
    (1.0 / ((sample_rate * ramp_ms) / 1000.0).max(1.0)).min(1.0)
}

fn transition_gain_interpolate(from: f32, to: f32, t: f32) -> f32 {
    let from = from.clamp(0.0, 1.0);
    let to = to.clamp(0.0, 1.0);
    let t = t.clamp(0.0, 1.0);
    let from_power = from * from;
    let to_power = to * to;
    let power = from_power + (to_power - from_power) * t;
    power.max(0.0).sqrt().clamp(0.0, 1.0)
}

impl OutputSinkWorker {
    pub(crate) fn start(args: OutputSinkWorkerStartArgs) -> Result<Self, String> {
        let OutputSinkWorkerStartArgs {
            plugin_id,
            type_id,
            target_json,
            config_json,
            channels,
            sample_rate,
            volume,
            transition_gain,
            transition_target_gain,
            transition_ramp_ms,
            internal_tx,
        } = args;
        let (tx, rx) =
            crossbeam_channel::bounded::<OutputSinkWrite>(OUTPUT_SINK_QUEUE_CAP_MESSAGES);
        let (ctrl_tx, ctrl_rx) = crossbeam_channel::unbounded::<OutputSinkControl>();
        let pending_samples = Arc::new(AtomicUsize::new(0));
        let pending_samples_for_thread = Arc::clone(&pending_samples);
        let sink_runtime_queued_samples = Arc::new(AtomicUsize::new(0));
        let sink_runtime_queued_samples_for_thread = Arc::clone(&sink_runtime_queued_samples);
        let (startup_tx, startup_rx) = crossbeam_channel::bounded::<Result<(), String>>(1);
        let join = std::thread::Builder::new()
            .name("stellatune-output-sink".to_string())
            .spawn(move || {
                let _rt_guard = stellatune_output::enable_realtime_audio_thread();
                let (mut controller, worker_control_rx) = match create_output_sink_controller_and_open(
                    &plugin_id,
                    &type_id,
                    &config_json,
                    &target_json,
                    sample_rate,
                    channels,
                ) {
                    Ok(sink) => {
                        let _ = startup_tx.send(Ok(()));
                        sink
                    }
                    Err(err) => {
                        let _ = startup_tx.send(Err(err));
                        return;
                    }
                };
                let mut current_config_json = config_json;
                let mut master_gain = MasterGainProcessor::new(
                    volume,
                    transition_gain,
                    transition_target_gain,
                    transition_ramp_ms,
                    sample_rate,
                );
                loop {
                    crossbeam_channel::select! {
                        recv(ctrl_rx) -> msg => {
                            let Ok(msg) = msg else {
                                break;
                            };
                            match msg {
                                OutputSinkControl::UpdateConfig { config_json, resp_tx } => {
                                    if config_json == current_config_json {
                                        let _ = resp_tx.send(Ok(()));
                                        continue;
                                    }
                                    let update_outcome = match controller.apply_config_update(config_json.clone()) {
                                        Ok(v) => v,
                                        Err(e) => {
                                            let _ = resp_tx.send(Err(format!(
                                                "output sink apply_config_update failed: {e}"
                                            )));
                                            continue;
                                        }
                                    };

                                    match update_outcome {
                                        WorkerConfigUpdateOutcome::Applied { revision: generation } => {
                                            emit_config_update_runtime_event(
                                                &plugin_id,
                                                "output_sink",
                                                &type_id,
                                                "applied",
                                                generation,
                                                None,
                                            );
                                            current_config_json = config_json;
                                            let _ = resp_tx.send(Ok(()));
                                        }
                                        WorkerConfigUpdateOutcome::RequiresRecreate { revision: generation, reason } => {
                                            emit_config_update_runtime_event(
                                                &plugin_id,
                                                "output_sink",
                                                &type_id,
                                                "requires_recreate",
                                                generation,
                                                reason.as_deref(),
                                            );
                                            match recreate_output_sink_instance(
                                                &plugin_id,
                                                &type_id,
                                                &target_json,
                                                sample_rate,
                                                channels,
                                                &mut controller,
                                            ) {
                                                Ok(()) => {
                                                    current_config_json = config_json;
                                                    emit_config_update_runtime_event(
                                                        &plugin_id,
                                                        "output_sink",
                                                        &type_id,
                                                        "recreated",
                                                        generation,
                                                        None,
                                                    );
                                                    if let Some(reason) = reason {
                                                        debug!(plugin_id, type_id, "output sink worker recreate: {reason}");
                                                    }
                                                    let _ = resp_tx.send(Ok(()));
                                                }
                                                Err(e) => {
                                                    emit_config_update_runtime_event(
                                                        &plugin_id,
                                                        "output_sink",
                                                        &type_id,
                                                        "failed",
                                                        generation,
                                                        Some(&e),
                                                    );
                                                    let _ = resp_tx.send(Err(format!(
                                                        "output sink recreate failed: {e}"
                                                    )));
                                                }
                                            }
                                        }
                                        WorkerConfigUpdateOutcome::DeferredNoInstance => {
                                            let generation = current_plugin_lease_id(&plugin_id);
                                            emit_config_update_runtime_event(
                                                &plugin_id,
                                                "output_sink",
                                                &type_id,
                                                "requires_recreate",
                                                generation,
                                                Some("deferred_no_instance"),
                                            );
                                            match recreate_output_sink_instance(
                                                &plugin_id,
                                                &type_id,
                                                &target_json,
                                                sample_rate,
                                                channels,
                                                &mut controller,
                                            ) {
                                                Ok(()) => {
                                                    current_config_json = config_json;
                                                    emit_config_update_runtime_event(
                                                        &plugin_id,
                                                        "output_sink",
                                                        &type_id,
                                                        "recreated",
                                                        generation,
                                                        Some("deferred_no_instance"),
                                                    );
                                                    let _ = resp_tx.send(Ok(()));
                                                }
                                                Err(e) => {
                                                    emit_config_update_runtime_event(
                                                        &plugin_id,
                                                        "output_sink",
                                                        &type_id,
                                                        "failed",
                                                        generation,
                                                        Some(&e),
                                                    );
                                                    let _ = resp_tx.send(Err(format!(
                                                        "output sink recreate failed: {e}"
                                                    )));
                                                }
                                            }
                                        }
                                        WorkerConfigUpdateOutcome::Rejected { revision: generation, reason } => {
                                            emit_config_update_runtime_event(
                                                &plugin_id,
                                                "output_sink",
                                                &type_id,
                                                "rejected",
                                                generation,
                                                Some(&reason),
                                            );
                                            let _ = resp_tx.send(Err(format!(
                                                "output sink config update rejected: {reason}"
                                            )));
                                        }
                                        WorkerConfigUpdateOutcome::Failed { revision: generation, error } => {
                                            emit_config_update_runtime_event(
                                                &plugin_id,
                                                "output_sink",
                                                &type_id,
                                                "failed",
                                                generation,
                                                Some(&error),
                                            );
                                            let _ = resp_tx.send(Err(format!(
                                                "output sink config update failed: {error}"
                                            )));
                                        }
                                    }
                                }
                                OutputSinkControl::ResetForDisrupt { resp_tx } => {
                                    let mut dropped_samples = 0usize;
                                    while let Ok(pending_msg) = rx.try_recv() {
                                        match pending_msg {
                                            OutputSinkWrite::Samples(samples) => {
                                                dropped_samples = dropped_samples
                                                    .saturating_add(samples.len());
                                            }
                                            OutputSinkWrite::Shutdown { drain } => {
                                                if let Some(sink) = controller.instance_mut() {
                                                    if drain {
                                                        let _ = sink.flush();
                                                    }
                                                    sink.close();
                                                }
                                                let _ = resp_tx.send(Ok(()));
                                                return;
                                            }
                                        }
                                    }
                                    if dropped_samples > 0 {
                                        let _ = pending_samples_for_thread.fetch_update(
                                            Ordering::Relaxed,
                                            Ordering::Relaxed,
                                            |current| Some(current.saturating_sub(dropped_samples)),
                                        );
                                    }

                                    let Some(sink) = controller.instance_mut() else {
                                        let _ = resp_tx.send(Err(
                                            "output sink reset_for_disrupt failed: instance missing"
                                                .to_string(),
                                        ));
                                        continue;
                                    };
                                    if let Err(e) = sink.reset() {
                                        let _ = resp_tx.send(Err(format!(
                                            "output sink reset_for_disrupt failed: {e}"
                                        )));
                                        continue;
                                    }
                                    if let Ok(status) = sink.query_status() {
                                        sink_runtime_queued_samples_for_thread
                                            .store(status.queued_samples as usize, Ordering::Release);
                                    }

                                    let _ = resp_tx.send(Ok(()));
                                }
                            }
                        }
                        recv(worker_control_rx) -> msg => {
                            let Ok(msg) = msg else {
                                break;
                            };
                            controller.on_control_message(msg);

                            if controller.has_pending_destroy() {
                                let _ = internal_tx.try_send(internal_output_error_dispatch(
                                    format!(
                                        "plugin sink destroyed by runtime control: {}::{}",
                                        plugin_id, type_id
                                    ),
                                ));
                                break;
                            }

                            if !controller.has_pending_recreate() {
                                continue;
                            }

                            let generation = current_plugin_lease_id(&plugin_id);
                            match recreate_output_sink_instance(
                                &plugin_id,
                                &type_id,
                                &target_json,
                                sample_rate,
                                channels,
                                &mut controller,
                            ) {
                                Ok(()) => {
                                    emit_config_update_runtime_event(
                                        &plugin_id,
                                        "output_sink",
                                        &type_id,
                                        "recreated",
                                        generation,
                                        Some("worker_control:recreate"),
                                    );
                                }
                                Err(e) => {
                                    emit_config_update_runtime_event(
                                        &plugin_id,
                                        "output_sink",
                                        &type_id,
                                        "failed",
                                        generation,
                                        Some(&e),
                                    );
                                    let _ = internal_tx.try_send(internal_output_error_dispatch(
                                        format!(
                                            "plugin sink recreate by runtime control failed: {e}"
                                        ),
                                    ));
                                    break;
                                }
                            }
                        }
                        recv(rx) -> msg => {
                            let Ok(msg) = msg else {
                                break;
                            };
                            match msg {
                                OutputSinkWrite::Samples(mut samples) => {
                                    let queued = samples.len();
                                    if samples.is_empty() {
                                        continue;
                                    }
                                    master_gain.apply_in_place(&mut samples);
                                    let Some(sink) = controller.instance_mut() else {
                                        let _ = pending_samples_for_thread.fetch_update(
                                            Ordering::Relaxed,
                                            Ordering::Relaxed,
                                            |current| Some(current.saturating_sub(queued)),
                                        );
                                        let _ = internal_tx.try_send(
                                            internal_output_error_dispatch(
                                                "plugin sink instance missing".to_string(),
                                            ),
                                        );
                                        break;
                                    };
                                    if let Err(e) = write_all_frames(sink, channels, &samples) {
                                        let _ = pending_samples_for_thread.fetch_update(
                                            Ordering::Relaxed,
                                            Ordering::Relaxed,
                                            |current| Some(current.saturating_sub(queued)),
                                        );
                                        warn!("output sink write failed: {e:#}");
                                        let _ = internal_tx.try_send(
                                            internal_output_error_dispatch(format!(
                                                "plugin sink write failed: {e}"
                                            )),
                                        );
                                        break;
                                    }
                                    if let Ok(status) = sink.query_status() {
                                        sink_runtime_queued_samples_for_thread
                                            .store(status.queued_samples as usize, Ordering::Release);
                                    }
                                    let _ = pending_samples_for_thread.fetch_update(
                                        Ordering::Relaxed,
                                        Ordering::Relaxed,
                                        |current| Some(current.saturating_sub(queued)),
                                    );
                                }
                                OutputSinkWrite::Shutdown { drain } => {
                                    if let Some(sink) = controller.instance_mut() {
                                        if drain {
                                            let _ = sink.flush();
                                        }
                                        sink.close();
                                    }
                                    break;
                                }
                            }
                        }
                    }
                }
            })
            .expect("failed to spawn stellatune-output-sink thread");
        match startup_rx.recv() {
            Ok(Ok(())) => {}
            Ok(Err(err)) => {
                let _ = join.join();
                return Err(err);
            }
            Err(_) => {
                let _ = join.join();
                return Err("output sink worker startup channel closed".to_string());
            }
        }
        Ok(Self {
            tx,
            ctrl_tx,
            pending_samples,
            sink_runtime_queued_samples,
            join,
        })
    }

    pub(crate) fn sender(&self) -> OutputSinkTx {
        OutputSinkTx::new(self.tx.clone(), Arc::clone(&self.pending_samples))
    }

    pub(crate) fn pending_samples(&self) -> usize {
        self.pending_samples.load(Ordering::Relaxed)
    }

    pub(crate) fn sink_runtime_queued_samples(&self) -> usize {
        self.sink_runtime_queued_samples.load(Ordering::Acquire)
    }

    pub(crate) fn apply_config_json(&self, config_json: String) -> Result<(), String> {
        let (resp_tx, resp_rx) = crossbeam_channel::bounded(1);
        self.ctrl_tx
            .send(OutputSinkControl::UpdateConfig {
                config_json,
                resp_tx,
            })
            .map_err(|_| "output sink worker exited".to_string())?;
        resp_rx
            .recv()
            .map_err(|_| "output sink worker dropped config update response".to_string())?
    }

    pub(crate) fn reset_for_disrupt(&self) -> Result<(), String> {
        let (resp_tx, resp_rx) = crossbeam_channel::bounded(1);
        self.ctrl_tx
            .send(OutputSinkControl::ResetForDisrupt { resp_tx })
            .map_err(|_| "output sink worker exited".to_string())?;
        resp_rx
            .recv()
            .map_err(|_| "output sink worker dropped disrupt reset response".to_string())?
    }

    pub(crate) fn shutdown(self, drain: bool) {
        let _ = self.tx.send(OutputSinkWrite::Shutdown { drain });
        let _ = self.join.join();
    }
}

fn recreate_output_sink_instance(
    plugin_id: &str,
    type_id: &str,
    target_json: &str,
    sample_rate: u32,
    channels: u16,
    controller: &mut OutputSinkWorkerController,
) -> Result<(), String> {
    let state_json = controller
        .instance()
        .and_then(|instance| instance.export_state_json().ok().flatten());
    controller.request_recreate();
    match controller.apply_pending().map_err(|e| e.to_string())? {
        WorkerApplyPendingOutcome::Created | WorkerApplyPendingOutcome::Recreated => {}
        WorkerApplyPendingOutcome::Destroyed | WorkerApplyPendingOutcome::Idle => {
            return Err(format!(
                "output sink recreate failed for {}::{}: controller has no instance",
                plugin_id, type_id
            ));
        }
    }
    let Some(sink) = controller.instance_mut() else {
        return Err(format!(
            "output sink recreate failed for {}::{}: controller has no instance",
            plugin_id, type_id
        ));
    };
    sink.open(
        target_json,
        StAudioSpec {
            sample_rate: sample_rate.max(1),
            channels: channels.max(1),
            reserved: 0,
        },
    )
    .map_err(|e| format!("output sink reopen failed: {e}"))?;
    if let Some(state_json) = state_json {
        let _ = sink.import_state_json(&state_json);
    }
    Ok(())
}

fn create_output_sink_controller_and_open(
    plugin_id: &str,
    type_id: &str,
    config_json: &str,
    target_json: &str,
    sample_rate: u32,
    channels: u16,
) -> Result<(OutputSinkWorkerController, Receiver<WorkerControlMessage>), String> {
    let endpoint = stellatune_runtime::block_on(
        stellatune_plugins::runtime::handle::shared_runtime_service()
            .bind_output_sink_worker_endpoint(plugin_id, type_id),
    )
    .map_err(|e| format!("bind_output_sink_worker_endpoint failed: {e}"))?;
    let (mut controller, control_rx) = endpoint.into_controller(config_json.to_string());
    match controller.apply_pending().map_err(|e| e.to_string())? {
        WorkerApplyPendingOutcome::Created | WorkerApplyPendingOutcome::Recreated => {}
        WorkerApplyPendingOutcome::Destroyed | WorkerApplyPendingOutcome::Idle => {
            return Err(format!(
                "create_output_sink_instance failed for {}::{}: controller has no instance",
                plugin_id, type_id
            ));
        }
    }
    let Some(sink) = controller.instance_mut() else {
        return Err(format!(
            "create_output_sink_instance failed for {}::{}: controller has no instance",
            plugin_id, type_id
        ));
    };
    sink.open(
        target_json,
        StAudioSpec {
            sample_rate: sample_rate.max(1),
            channels: channels.max(1),
            reserved: 0,
        },
    )
    .map_err(|e| format!("output sink reopen failed: {e}"))?;
    Ok((controller, control_rx))
}

fn current_plugin_lease_id(plugin_id: &str) -> u64 {
    stellatune_runtime::block_on(
        stellatune_plugins::runtime::handle::shared_runtime_service()
            .current_plugin_lease_info(plugin_id),
    )
    .map(|v| v.lease_id)
    .unwrap_or(0)
}

fn write_all_frames(
    sink: &mut stellatune_plugins::capabilities::output::OutputSinkInstance,
    channels: u16,
    samples: &[f32],
) -> Result<(), String> {
    let channels = channels.max(1) as usize;
    if channels == 0 || samples.is_empty() {
        return Ok(());
    }
    let mut offset = 0usize;
    let mut zero_accept_since: Option<Instant> = None;
    while offset < samples.len() {
        let frames_accepted = sink
            .write_interleaved_f32(channels as u16, &samples[offset..])
            .map_err(|e| e.to_string())?;
        let accepted_samples = frames_accepted as usize * channels;
        if accepted_samples == 0 {
            let started = *zero_accept_since.get_or_insert_with(Instant::now);
            if started.elapsed() >= Duration::from_millis(OUTPUT_SINK_WRITE_STALL_TIMEOUT_MS) {
                let remaining_frames = (samples.len().saturating_sub(offset)) / channels;
                return Err(format!(
                    "output sink stalled: accepted 0 frames for {}ms (remaining_frames={remaining_frames})",
                    OUTPUT_SINK_WRITE_STALL_TIMEOUT_MS
                ));
            }
            std::thread::sleep(Duration::from_millis(OUTPUT_SINK_WRITE_RETRY_SLEEP_MS));
            continue;
        }
        zero_accept_since = None;
        offset = offset.saturating_add(accepted_samples.min(samples.len() - offset));
    }
    Ok(())
}
