use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use stellatune_audio_core::pipeline::context::InputRef;
use stellatune_audio_core::pipeline::error::PipelineError;
use stellatune_runtime::thread_actor::{ActorRef, spawn_actor_named};

use crate::assembly::{AssembledPipeline, PipelineAssembler, PipelinePlan, PipelineRuntime};
use crate::control::actor::ControlActor;
use crate::control::messages::{
    ApplyStageControlMessage, GetSnapshotMessage, InstallDecodeWorkerMessage,
    OnDecodeLoopEventMessage, SetDspChainMessage, SetLfeModeMessage, SetResampleQualityMessage,
    SetVolumeMessage, ShutdownMessage, StopMessage,
};
use crate::event_hub::EventHub;
use crate::types::{
    DspChainSpec, EngineConfig, LfeMode, PlayerState, ResampleQuality, StopBehavior,
};
use crate::worker::decode_loop::{DecodeLoopEvent, DecodeLoopEventCallback, DecodeLoopWorker};

const TEST_TIMEOUT: Duration = Duration::from_millis(500);

fn test_config() -> EngineConfig {
    let mut config = EngineConfig::default();
    config.command_timeout = TEST_TIMEOUT;
    config.decode_command_timeout = TEST_TIMEOUT;
    config
}

#[derive(Default)]
struct DummyAssembler;

impl PipelineAssembler for DummyAssembler {
    fn plan(&self, _input: &InputRef) -> Result<Arc<dyn PipelinePlan>, PipelineError> {
        Err(PipelineError::StageFailure(
            "dummy assembler plan should not be called in control tests".to_string(),
        ))
    }

    fn create_runtime(&self) -> Box<dyn PipelineRuntime> {
        Box::new(DummyRuntime)
    }
}

struct DummyRuntime;

impl PipelineRuntime for DummyRuntime {
    fn ensure(&mut self, _plan: &dyn PipelinePlan) -> Result<AssembledPipeline, PipelineError> {
        Err(PipelineError::StageFailure(
            "dummy runtime ensure should not be called in control tests".to_string(),
        ))
    }

    fn apply_dsp_chain(&mut self, _spec: DspChainSpec) -> Result<(), PipelineError> {
        Ok(())
    }
}

fn spawn_control_actor(config: EngineConfig) -> (ActorRef<ControlActor>, JoinHandle<()>) {
    let actor = ControlActor::new(Arc::new(EventHub::new(config.event_capacity)), config);
    spawn_actor_named(actor, "stellatune-audio-v2-control-test")
        .expect("failed to spawn control actor")
}

fn install_decode_worker(actor_ref: &ActorRef<ControlActor>, config: &EngineConfig) {
    let assembler: Arc<dyn PipelineAssembler> = Arc::new(DummyAssembler);
    let callback: DecodeLoopEventCallback = Arc::new(|_| {});
    let worker = DecodeLoopWorker::start(assembler, config.clone(), callback);
    actor_ref
        .call(InstallDecodeWorkerMessage { worker }, TEST_TIMEOUT)
        .expect("failed to call install decode worker")
        .expect("failed to install decode worker");
}

fn shutdown_and_join(actor_ref: ActorRef<ControlActor>, join: JoinHandle<()>) {
    actor_ref
        .call(ShutdownMessage, TEST_TIMEOUT)
        .expect("failed to call shutdown")
        .expect("failed to shutdown control actor");
    drop(actor_ref);
    join.join().expect("failed to join control actor thread");
}

#[test]
fn stop_clears_current_track_in_snapshot() {
    let config = test_config();
    let (actor_ref, join) = spawn_control_actor(config.clone());
    install_decode_worker(&actor_ref, &config);

    actor_ref
        .cast(OnDecodeLoopEventMessage {
            event: DecodeLoopEvent::TrackChanged {
                track_token: "track-a".to_string(),
            },
        })
        .expect("failed to cast track changed event");
    let snapshot = actor_ref
        .call(GetSnapshotMessage, TEST_TIMEOUT)
        .expect("failed to call get snapshot");
    assert_eq!(snapshot.current_track.as_deref(), Some("track-a"));

    actor_ref
        .call(
            StopMessage {
                behavior: StopBehavior::Immediate,
            },
            TEST_TIMEOUT,
        )
        .expect("failed to call stop")
        .expect("stop failed");
    let snapshot = actor_ref
        .call(GetSnapshotMessage, TEST_TIMEOUT)
        .expect("failed to call get snapshot");
    assert_eq!(snapshot.state, PlayerState::Stopped);
    assert_eq!(snapshot.current_track, None);
    assert_eq!(snapshot.position_ms, 0);

    shutdown_and_join(actor_ref, join);
}

#[test]
fn eof_event_clears_current_track_and_resets_position() {
    let config = test_config();
    let (actor_ref, join) = spawn_control_actor(config);

    actor_ref
        .cast(OnDecodeLoopEventMessage {
            event: DecodeLoopEvent::TrackChanged {
                track_token: "track-a".to_string(),
            },
        })
        .expect("failed to cast track changed event");
    actor_ref
        .cast(OnDecodeLoopEventMessage {
            event: DecodeLoopEvent::StateChanged(PlayerState::Playing),
        })
        .expect("failed to cast state changed event");
    actor_ref
        .cast(OnDecodeLoopEventMessage {
            event: DecodeLoopEvent::Position { position_ms: 4800 },
        })
        .expect("failed to cast position event");
    actor_ref
        .cast(OnDecodeLoopEventMessage {
            event: DecodeLoopEvent::Eof,
        })
        .expect("failed to cast eof event");

    let snapshot = actor_ref
        .call(GetSnapshotMessage, TEST_TIMEOUT)
        .expect("failed to call get snapshot");
    assert_eq!(snapshot.state, PlayerState::Stopped);
    assert_eq!(snapshot.current_track, None);
    assert_eq!(snapshot.position_ms, 0);

    shutdown_and_join(actor_ref, join);
}

#[test]
fn error_event_clears_current_track_and_stops_state() {
    let config = test_config();
    let (actor_ref, join) = spawn_control_actor(config);

    actor_ref
        .cast(OnDecodeLoopEventMessage {
            event: DecodeLoopEvent::TrackChanged {
                track_token: "track-a".to_string(),
            },
        })
        .expect("failed to cast track changed event");
    actor_ref
        .cast(OnDecodeLoopEventMessage {
            event: DecodeLoopEvent::StateChanged(PlayerState::Playing),
        })
        .expect("failed to cast state changed event");
    actor_ref
        .cast(OnDecodeLoopEventMessage {
            event: DecodeLoopEvent::Position { position_ms: 6400 },
        })
        .expect("failed to cast position event");
    actor_ref
        .cast(OnDecodeLoopEventMessage {
            event: DecodeLoopEvent::Error("decoder failed".to_string()),
        })
        .expect("failed to cast error event");

    let snapshot = actor_ref
        .call(GetSnapshotMessage, TEST_TIMEOUT)
        .expect("failed to call get snapshot");
    assert_eq!(snapshot.state, PlayerState::Stopped);
    assert_eq!(snapshot.current_track, None);
    assert_eq!(snapshot.position_ms, 6400);

    shutdown_and_join(actor_ref, join);
}

#[test]
fn set_volume_message_forwards_to_decode_loop() {
    let config = test_config();
    let (actor_ref, join) = spawn_control_actor(config.clone());
    install_decode_worker(&actor_ref, &config);

    actor_ref
        .call(SetVolumeMessage { volume: 0.35 }, TEST_TIMEOUT)
        .expect("failed to call set volume")
        .expect("set volume failed");

    shutdown_and_join(actor_ref, join);
}

#[test]
fn set_dsp_chain_message_forwards_to_decode_loop() {
    let config = test_config();
    let (actor_ref, join) = spawn_control_actor(config.clone());
    install_decode_worker(&actor_ref, &config);

    actor_ref
        .call(
            SetDspChainMessage {
                spec: crate::types::DspChainSpec {
                    items: vec![crate::types::DspChainItem {
                        plugin_id: "plugin-a".to_string(),
                        type_id: "eq".to_string(),
                        config_json: "{\"gain\":1.2}".to_string(),
                        stage: crate::types::DspChainStage::PreMix,
                    }],
                },
            },
            TEST_TIMEOUT,
        )
        .expect("failed to call set dsp chain")
        .expect("set dsp chain failed");

    shutdown_and_join(actor_ref, join);
}

#[test]
fn set_lfe_mode_message_forwards_to_decode_loop() {
    let config = test_config();
    let (actor_ref, join) = spawn_control_actor(config.clone());
    install_decode_worker(&actor_ref, &config);

    actor_ref
        .call(
            SetLfeModeMessage {
                mode: LfeMode::MixToFront,
            },
            TEST_TIMEOUT,
        )
        .expect("failed to call set lfe mode")
        .expect("set lfe mode failed");

    shutdown_and_join(actor_ref, join);
}

#[test]
fn set_resample_quality_message_forwards_to_decode_loop() {
    let config = test_config();
    let (actor_ref, join) = spawn_control_actor(config.clone());
    install_decode_worker(&actor_ref, &config);

    actor_ref
        .call(
            SetResampleQualityMessage {
                quality: ResampleQuality::Ultra,
            },
            TEST_TIMEOUT,
        )
        .expect("failed to call set resample quality")
        .expect("set resample quality failed");

    shutdown_and_join(actor_ref, join);
}

#[test]
fn apply_stage_control_message_reaches_decode_loop() {
    let config = test_config();
    let (actor_ref, join) = spawn_control_actor(config.clone());
    install_decode_worker(&actor_ref, &config);

    let result = actor_ref
        .call(
            ApplyStageControlMessage {
                stage_key: "builtin.transition_gain".to_string(),
                control: Box::new(123_u32),
            },
            TEST_TIMEOUT,
        )
        .expect("failed to call apply stage control");
    assert!(result.is_ok());

    shutdown_and_join(actor_ref, join);
}
