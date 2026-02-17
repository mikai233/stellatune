use crate::config::engine::{PauseBehavior, StopBehavior};
use crate::engine::handle::EngineHandle;
use crate::engine::messages::{
    PauseMessage, PlayMessage, QueueNextTrackMessage, SeekMessage, StopMessage, SwitchTrackMessage,
};
use crate::error::EngineError;

impl EngineHandle {
    /// Switches the active track to `track_token`.
    ///
    /// When `autoplay` is `true`, playback transitions to playing state after
    /// the switch completes. When `false`, the track is prepared but remains
    /// paused.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError`] when the control actor call times out, exits, or
    /// the decode worker rejects the switch request.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use stellatune_audio::engine::EngineHandle;
    ///
    /// # async fn demo(handle: &EngineHandle) -> Result<(), stellatune_audio::error::EngineError> {
    /// handle
    ///     .switch_track_token("track-token".to_string(), true)
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn switch_track_token(
        &self,
        track_token: String,
        autoplay: bool,
    ) -> Result<(), EngineError> {
        self.actor_ref
            .call_async(
                SwitchTrackMessage {
                    track_token,
                    autoplay,
                },
                self.timeout,
            )
            .await
            .map_err(|error| Self::map_call_error("switch_track_token", self.timeout, error))?
    }

    /// Queues the next track token for EOF promotion.
    ///
    /// The currently active track is not interrupted. The queued token is
    /// consumed when the current decode session reaches EOF.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError`] when the control actor call fails or the decode
    /// worker cannot prewarm/accept the queued input.
    pub async fn queue_next_track_token(&self, track_token: String) -> Result<(), EngineError> {
        self.actor_ref
            .call_async(QueueNextTrackMessage { track_token }, self.timeout)
            .await
            .map_err(|error| Self::map_call_error("queue_next_track_token", self.timeout, error))?
    }

    /// Starts or resumes playback.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError`] when the control actor call fails or there is no
    /// active pipeline that can transition to playing state.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use stellatune_audio::engine::EngineHandle;
    ///
    /// # async fn demo(handle: &EngineHandle) -> Result<(), stellatune_audio::error::EngineError> {
    /// handle.play().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn play(&self) -> Result<(), EngineError> {
        self.actor_ref
            .call_async(PlayMessage, self.timeout)
            .await
            .map_err(|error| Self::map_call_error("play", self.timeout, error))?
    }

    /// Pauses playback immediately.
    ///
    /// This is equivalent to `pause_with(PauseBehavior::Immediate)`.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError`] when the control actor call fails.
    pub async fn pause(&self) -> Result<(), EngineError> {
        self.pause_with(PauseBehavior::Immediate).await
    }

    /// Pauses playback with the given pause behavior.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError`] when the control actor call fails or the decode
    /// worker cannot satisfy the requested pause behavior.
    pub async fn pause_with(&self, behavior: PauseBehavior) -> Result<(), EngineError> {
        self.actor_ref
            .call_async(PauseMessage { behavior }, self.timeout)
            .await
            .map_err(|error| Self::map_call_error("pause", self.timeout, error))?
    }

    /// Seeks the active track to `position_ms`.
    ///
    /// Negative positions are interpreted by the decode worker according to its
    /// seek policy.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError`] when the control actor call fails or there is no
    /// active pipeline to seek.
    pub async fn seek_ms(&self, position_ms: i64) -> Result<(), EngineError> {
        self.actor_ref
            .call_async(SeekMessage { position_ms }, self.timeout)
            .await
            .map_err(|error| Self::map_call_error("seek_ms", self.timeout, error))?
    }

    /// Stops playback immediately.
    ///
    /// This is equivalent to `stop_with(StopBehavior::Immediate)`.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError`] when the control actor call fails.
    pub async fn stop(&self) -> Result<(), EngineError> {
        self.stop_with(StopBehavior::Immediate).await
    }

    /// Stops playback with the given stop behavior.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError`] when the control actor call fails or the decode
    /// worker cannot satisfy the requested stop behavior.
    pub async fn stop_with(&self, behavior: StopBehavior) -> Result<(), EngineError> {
        self.actor_ref
            .call_async(StopMessage { behavior }, self.timeout)
            .await
            .map_err(|error| Self::map_call_error("stop", self.timeout, error))?
    }
}
