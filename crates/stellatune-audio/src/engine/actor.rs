use std::sync::Arc;

use lattice_actor::{
    actor_behavior,
    context::{ActorContext, HandlerContext},
    error::{ActorError, ActorStopError},
    traits::{Actor, StopReason},
};

use crate::config::engine::{EngineConfig, EngineSnapshot, Event, PlaybackState};
use crate::error::EngineError;
use crate::infra::event_hub::EventHub;
use crate::pipeline::assembly::PipelineFactory;
use crate::pipeline::runtime::dsp::control::SharedMasterGainHotControl;
use crate::workers::decode::DecodeWorkerEventCallback;

use super::messages::{
    AbortPluginChangeMessage, CompletePluginChangeMessage, GetSnapshotMessage,
    OnDecodeWorkerEventMessage, PauseMessage, PlayMessage, PumpAudioMessage, QueueNextTrackMessage,
    RebuildPipelineMessage, SeekMessage, SetLfeModeMessage, SetResampleQualityMessage,
    ShutdownMessage, StopMessage, SuspendForPluginChangeMessage, SwitchTrackMessage,
};
use super::session::PlaybackSession;
use crate::pipeline::plan::PlaybackCheckpoint;

actor_behavior! {
    PlaybackState {
        always => [
            GetSnapshotMessage,
            OnDecodeWorkerEventMessage,
            RebuildPipelineMessage,
            SetLfeModeMessage,
            SetResampleQualityMessage,
            ShutdownMessage,
            StopMessage
        ];
        PlaybackState::Idle => [SuspendForPluginChangeMessage, SwitchTrackMessage];
        PlaybackState::Preparing => [SuspendForPluginChangeMessage, SwitchTrackMessage];
        PlaybackState::Ready => [PlayMessage, QueueNextTrackMessage, SeekMessage, SuspendForPluginChangeMessage, SwitchTrackMessage];
        PlaybackState::Playing => [PlayMessage, PauseMessage, PumpAudioMessage, QueueNextTrackMessage, SeekMessage, SuspendForPluginChangeMessage, SwitchTrackMessage];
        PlaybackState::Paused => [PlayMessage, PauseMessage, QueueNextTrackMessage, SeekMessage, SuspendForPluginChangeMessage, SwitchTrackMessage];
        PlaybackState::Draining => [SuspendForPluginChangeMessage];
        PlaybackState::Recovering => [PumpAudioMessage, SuspendForPluginChangeMessage];
        PlaybackState::Reconfiguring => [AbortPluginChangeMessage, CompletePluginChangeMessage, PauseMessage, PlayMessage, QueueNextTrackMessage, SeekMessage, SwitchTrackMessage];
    }
}

pub(crate) struct PlaybackActor {
    pub(crate) events: Arc<EventHub>,
    pub(crate) config: EngineConfig,
    pub(crate) current_track: Option<String>,
    pub(crate) position_ms: i64,
    pub(crate) session: Option<PlaybackSession>,
    factory: Option<Arc<dyn PipelineFactory>>,
    master_gain_hot_control: SharedMasterGainHotControl,
    pub(crate) pump_scheduled: bool,
    pub(crate) plugin_checkpoint: Option<PlaybackCheckpoint>,
}

impl PlaybackActor {
    pub(crate) fn new(
        events: Arc<EventHub>,
        config: EngineConfig,
        factory: Arc<dyn PipelineFactory>,
        master_gain_hot_control: SharedMasterGainHotControl,
    ) -> Self {
        Self {
            events,
            config,
            current_track: None,
            position_ms: 0,
            session: None,
            factory: Some(factory),
            master_gain_hot_control,
            pump_scheduled: false,
            plugin_checkpoint: None,
        }
    }

    pub(crate) fn ensure_session(&mut self) -> Result<&mut PlaybackSession, EngineError> {
        self.session.as_mut().ok_or(EngineError::WorkerNotInstalled)
    }

    pub(crate) fn emit_error(&self, message: String) {
        self.events.emit(Event::Error { message });
    }

    pub(crate) fn snapshot(&self, state: PlaybackState) -> EngineSnapshot {
        EngineSnapshot {
            state: state.public_state(),
            current_track: self.current_track.clone(),
            position_ms: self.position_ms,
        }
    }

    pub(crate) fn transition_state(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        next: PlaybackState,
    ) {
        let previous_public = ctx.behavior().public_state();
        if *ctx.behavior() == next {
            return;
        }
        ctx.transition_to(next);
        let next_public = next.public_state();
        if previous_public != next_public {
            self.events.emit(Event::StateChanged { state: next_public });
        }
    }

    pub(crate) fn update_position(&mut self, position_ms: i64) {
        self.position_ms = position_ms.max(0);
        self.events.emit(Event::Position {
            position_ms: self.position_ms,
        });
    }

    pub(crate) fn schedule_pump(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        delay: std::time::Duration,
    ) {
        if self.pump_scheduled {
            return;
        }
        self.pump_scheduled = true;
        ctx.notify_after(delay, PumpAudioMessage);
    }
}

impl Actor for PlaybackActor {
    type Error = ActorError;
    type Behavior = PlaybackState;

    async fn started(&mut self, ctx: &mut ActorContext<Self>) -> Result<(), ActorError> {
        let factory = self
            .factory
            .take()
            .ok_or_else(|| ActorError::new("playback factory was already consumed"))?;
        let actor_handle = ctx.self_handle();
        let callback: DecodeWorkerEventCallback = Arc::new(move |event| {
            if actor_handle
                .try_tell(OnDecodeWorkerEventMessage { event })
                .is_err()
            {
                tracing::error!("playback actor rejected a decode-worker event");
            }
        });
        self.session = Some(PlaybackSession::new(
            factory,
            self.config.clone(),
            callback,
            Arc::clone(&self.master_gain_hot_control),
        ));
        Ok(())
    }

    async fn stopping(
        &mut self,
        _ctx: &mut ActorContext<Self>,
        _reason: StopReason,
    ) -> Result<(), ActorStopError> {
        if let Some(session) = self.session.take() {
            session
                .shutdown(self.config.decode_command_timeout)
                .map_err(|error| ActorStopError::new(error.to_string()))?;
        }
        Ok(())
    }
}
