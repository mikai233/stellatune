use std::sync::Arc;

use lattice_actor::{
    mailbox::MailboxConfig,
    runtime::{ActorExecutionPolicy, ActorRuntime, ActorSpawnOptions},
};

use crate::config::engine::EngineConfig;
use crate::error::EngineError;
use crate::infra::event_hub::EventHub;
use crate::pipeline::assembly::PipelineFactory;
use crate::pipeline::runtime::dsp::control::MasterGainHotControl;
use lattice_actor::traits::ActorLifecycleState;

use crate::engine::actor::PlaybackActor;
use crate::engine::handle::EngineHandle;

pub(crate) fn start_engine(factory: Arc<dyn PipelineFactory>) -> Result<EngineHandle, EngineError> {
    start_engine_with_config(factory, EngineConfig::default())
}

pub(crate) fn start_engine_with_config(
    factory: Arc<dyn PipelineFactory>,
    config: EngineConfig,
) -> Result<EngineHandle, EngineError> {
    let events = Arc::new(EventHub::new(config.event_capacity));
    let master_gain_hot_control = Arc::new(MasterGainHotControl::default());
    let actor = PlaybackActor::new(
        Arc::clone(&events),
        config.clone(),
        factory,
        Arc::clone(&master_gain_hot_control),
    );
    let actor_ref = ActorRuntime::default()
        .spawn_actor(
            actor,
            ActorSpawnOptions {
                mailbox: MailboxConfig::bounded(config.decode_command_capacity),
                execution: Some(ActorExecutionPolicy::DedicatedThreadPool { worker_count: 1 }),
                ..ActorSpawnOptions::default()
            },
        )
        .map_err(|error| EngineError::SpawnPlaybackActor {
            message: error.to_string(),
        })?;

    let start_deadline = std::time::Instant::now() + config.command_timeout;
    while actor_ref.lifecycle_state() == ActorLifecycleState::Starting {
        if std::time::Instant::now() >= start_deadline {
            return Err(EngineError::ControlCommandTimedOut {
                operation: "start_playback_actor",
                timeout_ms: config.command_timeout.as_millis(),
            });
        }
        std::thread::yield_now();
    }
    if actor_ref.lifecycle_state() != ActorLifecycleState::Running {
        return Err(EngineError::ControlActorExited {
            operation: "start_playback_actor",
        });
    }

    Ok(EngineHandle::new(
        actor_ref,
        events,
        master_gain_hot_control,
        config.command_timeout,
    ))
}
