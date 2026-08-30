use tokio::sync::broadcast;

use crate::config::engine::{EngineSnapshot, Event};
use crate::engine::handle::EngineHandle;
use crate::engine::messages::{
    AbortPluginChangeMessage, CompletePluginChangeMessage, GetSnapshotMessage,
    RebuildPipelineMessage, ShutdownMessage, SuspendForPluginChangeMessage,
};
use crate::error::EngineError;
use crate::pipeline::plan::PlaybackCheckpoint;

impl EngineHandle {
    /// Rebuilds the active pipeline from the latest typed registry and host
    /// configuration while preserving playback position.
    pub async fn rebuild_pipeline(&self) -> Result<(), EngineError> {
        self.actor_ref
            .ask(RebuildPipelineMessage, self.timeout)
            .await
            .map_err(|error| Self::map_call_error("rebuild_pipeline", self.timeout, error))?
    }

    /// Pauses playback, captures the sink-consumed position, and tears down the
    /// complete native session before a plugin package or registry change.
    pub async fn suspend_for_plugin_change(
        &self,
    ) -> Result<Option<PlaybackCheckpoint>, EngineError> {
        self.actor_ref
            .ask(SuspendForPluginChangeMessage, self.timeout)
            .await
            .map_err(|error| {
                Self::map_call_error("suspend_for_plugin_change", self.timeout, error)
            })?
    }

    /// Rebuilds from the latest registry/assembler state and restores the
    /// checkpoint captured by [`Self::suspend_for_plugin_change`].
    pub async fn complete_plugin_change(&self) -> Result<(), EngineError> {
        self.actor_ref
            .ask(CompletePluginChangeMessage, self.timeout)
            .await
            .map_err(|error| Self::map_call_error("complete_plugin_change", self.timeout, error))?
    }

    /// Aborts a plugin transaction and rebuilds the previous playback session.
    pub async fn abort_plugin_change(&self) -> Result<(), EngineError> {
        self.actor_ref
            .ask(AbortPluginChangeMessage, self.timeout)
            .await
            .map_err(|error| Self::map_call_error("abort_plugin_change", self.timeout, error))?
    }

    /// Returns the latest engine snapshot.
    ///
    /// The snapshot contains the current state, active track token, and
    /// position.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError`] when the control actor call fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use stellatune_audio::engine::EngineHandle;
    ///
    /// # async fn demo(handle: &EngineHandle) -> Result<(), stellatune_audio::error::EngineError> {
    /// let snapshot = handle.snapshot().await?;
    /// let _state = snapshot.state;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn snapshot(&self) -> Result<EngineSnapshot, EngineError> {
        self.actor_ref
            .ask(GetSnapshotMessage, self.timeout)
            .await
            .map_err(|error| Self::map_call_error("snapshot", self.timeout, error))
    }

    /// Gracefully shuts down the engine runtime.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError`] when the control actor call fails or shutdown
    /// acknowledgement cannot be completed in time.
    pub async fn shutdown(&self) -> Result<(), EngineError> {
        let result = self
            .actor_ref
            .ask(ShutdownMessage, self.timeout)
            .await
            .map_err(|error| Self::map_call_error("shutdown", self.timeout, error))?;
        result?;
        let mut terminated = self.actor_ref.subscribe_terminated();
        self.actor_ref
            .stop(lattice_actor::traits::StopReason::Requested)
            .map_err(|_| EngineError::ControlActorExited {
                operation: "shutdown",
            })?;
        tokio::time::timeout(self.timeout, terminated.recv())
            .await
            .map_err(|_| EngineError::ControlCommandTimedOut {
                operation: "shutdown",
                timeout_ms: self.timeout.as_millis(),
            })?
            .map_err(|_| EngineError::ControlActorExited {
                operation: "shutdown",
            })?;
        Ok(())
    }

    /// Subscribes to engine events.
    ///
    /// The returned receiver is a Tokio broadcast receiver. Slow consumers may
    /// observe lagged errors and should handle them explicitly.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::sync::broadcast::error::RecvError;
    /// use stellatune_audio::engine::EngineHandle;
    ///
    /// # async fn demo(handle: &EngineHandle) {
    /// let mut rx = handle.subscribe_events();
    /// match rx.recv().await {
    ///     Ok(_event) => {}
    ///     Err(RecvError::Lagged(_)) => {}
    ///     Err(RecvError::Closed) => {}
    /// }
    /// # }
    /// ```
    pub fn subscribe_events(&self) -> broadcast::Receiver<Event> {
        self.events.subscribe()
    }
}
