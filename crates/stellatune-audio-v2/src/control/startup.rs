use std::sync::Arc;

use stellatune_runtime::thread_actor::spawn_actor_named;

use crate::assembly::PipelineAssembler;
use crate::event_hub::EventHub;
use crate::types::EngineConfig;
use crate::worker::decode_loop::{DecodeLoopEventCallback, DecodeLoopWorker};

use crate::control::actor::ControlActor;
use crate::control::engine_handle::EngineHandle;
use crate::control::messages::{InstallDecodeWorkerMessage, OnDecodeLoopEventMessage};

pub(crate) fn start_engine(assembler: Arc<dyn PipelineAssembler>) -> Result<EngineHandle, String> {
    start_engine_with_config(assembler, EngineConfig::default())
}

pub(crate) fn start_engine_with_config(
    assembler: Arc<dyn PipelineAssembler>,
    config: EngineConfig,
) -> Result<EngineHandle, String> {
    let events = Arc::new(EventHub::new(config.event_capacity));
    let actor = ControlActor::new(Arc::clone(&events), config.clone());
    let (actor_ref, _join) = spawn_actor_named(actor, "stellatune-audio-v2-control")
        .map_err(|e| format!("failed to spawn control actor: {e}"))?;

    let worker_actor_ref = actor_ref.clone();
    let worker_callback: DecodeLoopEventCallback = Arc::new(move |event| {
        let _ = worker_actor_ref.cast(OnDecodeLoopEventMessage { event });
    });
    let worker = DecodeLoopWorker::start(assembler, config.clone(), worker_callback);

    actor_ref
        .call(
            InstallDecodeWorkerMessage { worker },
            config.command_timeout,
        )
        .map_err(EngineHandle::map_call_error)??;

    Ok(EngineHandle::new(actor_ref, events, config.command_timeout))
}
