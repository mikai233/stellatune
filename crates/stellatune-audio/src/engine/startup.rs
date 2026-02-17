use std::sync::Arc;

use stellatune_runtime::thread_actor::spawn_actor_named;

use crate::config::engine::EngineConfig;
use crate::error::EngineError;
use crate::infra::event_hub::EventHub;
use crate::pipeline::assembly::PipelineAssembler;
use crate::pipeline::runtime::dsp::control::MasterGainHotControl;
use crate::workers::decode::{DecodeWorker, DecodeWorkerEventCallback};

use crate::engine::actor::ControlActor;
use crate::engine::handle::EngineHandle;
use crate::engine::messages::{InstallDecodeWorkerMessage, OnDecodeWorkerEventMessage};

pub(crate) fn start_engine(
    assembler: Arc<dyn PipelineAssembler>,
) -> Result<EngineHandle, EngineError> {
    start_engine_with_config(assembler, EngineConfig::default())
}

pub(crate) fn start_engine_with_config(
    assembler: Arc<dyn PipelineAssembler>,
    config: EngineConfig,
) -> Result<EngineHandle, EngineError> {
    let events = Arc::new(EventHub::new(config.event_capacity));
    let master_gain_hot_control = Arc::new(MasterGainHotControl::default());
    let actor = ControlActor::new(Arc::clone(&events), config.clone());
    let (actor_ref, _join) = spawn_actor_named(actor, "stellatune-audio-control")
        .map_err(|source| EngineError::SpawnControlActor { source })?;

    let worker_actor_ref = actor_ref.clone();
    let worker_callback: DecodeWorkerEventCallback = Arc::new(move |event| {
        let _ = worker_actor_ref.cast(OnDecodeWorkerEventMessage { event });
    });
    let worker = DecodeWorker::start(
        assembler,
        config.clone(),
        worker_callback,
        Arc::clone(&master_gain_hot_control),
    );

    actor_ref
        .call(
            InstallDecodeWorkerMessage { worker },
            config.command_timeout,
        )
        .map_err(|error| {
            EngineError::from_call_error("install_decode_worker", config.command_timeout, error)
        })??;

    Ok(EngineHandle::new(
        actor_ref,
        events,
        master_gain_hot_control,
        config.command_timeout,
    ))
}
