use crate::config::engine::{PauseBehavior, StopBehavior};
use crate::engine::handle::EngineHandle;
use crate::engine::messages::{
    PauseMessage, PlayMessage, QueueNextTrackMessage, SeekMessage, StopMessage, SwitchTrackMessage,
};

impl EngineHandle {
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

    pub async fn stop(&self) -> Result<(), String> {
        self.stop_with(StopBehavior::Immediate).await
    }

    pub async fn stop_with(&self, behavior: StopBehavior) -> Result<(), String> {
        self.actor_ref
            .call_async(StopMessage { behavior }, self.timeout)
            .await
            .map_err(Self::map_call_error)?
    }
}
