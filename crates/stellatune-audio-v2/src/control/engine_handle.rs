use std::any::Any;
use std::sync::Arc;

use tokio::sync::broadcast;

use stellatune_runtime::thread_actor::{ActorRef, CallError};

use crate::assembly::{PipelineMutation, PipelinePlan};
use crate::event_hub::EventHub;
use crate::runtime::transform::control::SharedMasterGainHotControl;
use crate::types::{EngineSnapshot, Event, LfeMode, PauseBehavior, ResampleQuality, StopBehavior};

use crate::control::actor::ControlActor;
use crate::control::messages::{
    ApplyPipelineMutationMessage, ApplyPipelinePlanMessage, ApplyStageControlMessage,
    GetSnapshotMessage, PauseMessage, PlayMessage, QueueNextTrackMessage, SeekMessage,
    SetLfeModeMessage, SetResampleQualityMessage, ShutdownMessage, StopMessage, SwitchTrackMessage,
};

#[derive(Clone)]
pub struct EngineHandle {
    actor_ref: ActorRef<ControlActor>,
    events: Arc<EventHub>,
    master_gain_hot_control: SharedMasterGainHotControl,
    timeout: std::time::Duration,
}

impl EngineHandle {
    pub(crate) fn new(
        actor_ref: ActorRef<ControlActor>,
        events: Arc<EventHub>,
        master_gain_hot_control: SharedMasterGainHotControl,
        timeout: std::time::Duration,
    ) -> Self {
        Self {
            actor_ref,
            events,
            master_gain_hot_control,
            timeout,
        }
    }

    pub(crate) fn map_call_error(err: CallError) -> String {
        match err {
            CallError::MailboxClosed | CallError::ActorStopped => {
                "control actor exited".to_string()
            },
            CallError::Timeout => "control command timed out".to_string(),
        }
    }

    pub async fn switch_track_token(
        &self,
        track_token: String,
        autoplay: bool,
    ) -> Result<(), String> {
        self.actor_ref
            .call_async(
                SwitchTrackMessage {
                    track_token,
                    autoplay,
                },
                self.timeout,
            )
            .await
            .map_err(Self::map_call_error)?
    }

    pub async fn queue_next_track_token(&self, track_token: String) -> Result<(), String> {
        self.actor_ref
            .call_async(QueueNextTrackMessage { track_token }, self.timeout)
            .await
            .map_err(Self::map_call_error)?
    }

    pub async fn play(&self) -> Result<(), String> {
        self.actor_ref
            .call_async(PlayMessage, self.timeout)
            .await
            .map_err(Self::map_call_error)?
    }

    pub async fn pause(&self) -> Result<(), String> {
        self.pause_with(PauseBehavior::Immediate).await
    }

    pub async fn pause_with(&self, behavior: PauseBehavior) -> Result<(), String> {
        self.actor_ref
            .call_async(PauseMessage { behavior }, self.timeout)
            .await
            .map_err(Self::map_call_error)?
    }

    pub async fn seek_ms(&self, position_ms: i64) -> Result<(), String> {
        self.actor_ref
            .call_async(SeekMessage { position_ms }, self.timeout)
            .await
            .map_err(Self::map_call_error)?
    }

    pub async fn set_volume(&self, volume: f32, seq: u64, ramp_ms: u32) -> Result<(), String> {
        self.master_gain_hot_control.update(volume, ramp_ms, None);
        self.events.emit(Event::VolumeChanged { volume, seq });
        Ok(())
    }

    pub async fn set_lfe_mode(&self, mode: LfeMode) -> Result<(), String> {
        self.actor_ref
            .call_async(SetLfeModeMessage { mode }, self.timeout)
            .await
            .map_err(Self::map_call_error)?
    }

    pub async fn set_resample_quality(&self, quality: ResampleQuality) -> Result<(), String> {
        self.actor_ref
            .call_async(SetResampleQualityMessage { quality }, self.timeout)
            .await
            .map_err(Self::map_call_error)?
    }

    pub async fn apply_stage_control<T>(
        &self,
        stage_key: impl Into<String>,
        control: T,
    ) -> Result<(), String>
    where
        T: Any + Send + 'static,
    {
        self.actor_ref
            .call_async(
                ApplyStageControlMessage {
                    stage_key: stage_key.into(),
                    control: Box::new(control),
                },
                self.timeout,
            )
            .await
            .map_err(Self::map_call_error)?
    }

    pub async fn stop(&self) -> Result<(), String> {
        self.stop_with(StopBehavior::Immediate).await
    }

    pub async fn stop_with(&self, behavior: StopBehavior) -> Result<(), String> {
        self.actor_ref
            .call_async(StopMessage { behavior }, self.timeout)
            .await
            .map_err(Self::map_call_error)?
    }

    pub async fn apply_pipeline_plan(&self, plan: Arc<dyn PipelinePlan>) -> Result<(), String> {
        self.actor_ref
            .call_async(ApplyPipelinePlanMessage { plan }, self.timeout)
            .await
            .map_err(Self::map_call_error)?
    }

    pub async fn apply_pipeline_mutation(&self, mutation: PipelineMutation) -> Result<(), String> {
        self.actor_ref
            .call_async(ApplyPipelineMutationMessage { mutation }, self.timeout)
            .await
            .map_err(Self::map_call_error)?
    }

    pub async fn snapshot(&self) -> Result<EngineSnapshot, String> {
        self.actor_ref
            .call_async(GetSnapshotMessage, self.timeout)
            .await
            .map_err(Self::map_call_error)
    }

    pub async fn shutdown(&self) -> Result<(), String> {
        self.actor_ref
            .call_async(ShutdownMessage, self.timeout)
            .await
            .map_err(Self::map_call_error)?
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<Event> {
        self.events.subscribe()
    }
}
