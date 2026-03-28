use std::sync::Arc;

use tokio::sync::broadcast;

use crate::config::engine::{EngineSnapshot, Event};
use crate::engine::handle::EngineHandle;
use crate::engine::messages::{
    ApplyPipelineBlueprintMessage, ApplyPipelineMutationMessage, ApplyPipelineMutationsMessage,
    GetSnapshotMessage, ShutdownMessage,
};
use crate::error::EngineError;
use crate::pipeline::assembly::{PipelineBlueprint, PipelineMutation};

impl EngineHandle {
    /// Replaces the pinned pipeline blueprint used for subsequent opens/rebuilds.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError`] when the control actor call fails or the decode
    /// worker cannot apply the supplied blueprint.
    pub async fn apply_pipeline_blueprint(
        &self,
        blueprint: Arc<dyn PipelineBlueprint>,
    ) -> Result<(), EngineError> {
        self.actor_ref
            .call_async(ApplyPipelineBlueprintMessage { blueprint }, self.timeout)
            .await
            .map_err(|error| {
                Self::map_call_error("apply_pipeline_blueprint", self.timeout, error)
            })?
    }

    /// Applies a runtime pipeline mutation.
    ///
    /// Mutations update runtime policy such as transform graph layout, mixer
    /// plan, resampler plan, or built-in slot state.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError`] when the control actor call fails or the mutation
    /// is rejected by the runtime.
    pub async fn apply_pipeline_mutation(
        &self,
        mutation: PipelineMutation,
    ) -> Result<(), EngineError> {
        self.actor_ref
            .call_async(ApplyPipelineMutationMessage { mutation }, self.timeout)
            .await
            .map_err(|error| Self::map_call_error("apply_pipeline_mutation", self.timeout, error))?
    }

    /// Applies multiple runtime pipeline mutations as a single rebuild batch.
    ///
    /// Mutations are folded into the pinned blueprint first, then the active
    /// pipeline is rebuilt once from the final result.
    pub async fn apply_pipeline_mutations(
        &self,
        mutations: Vec<PipelineMutation>,
    ) -> Result<(), EngineError> {
        self.actor_ref
            .call_async(ApplyPipelineMutationsMessage { mutations }, self.timeout)
            .await
            .map_err(|error| {
                Self::map_call_error("apply_pipeline_mutations", self.timeout, error)
            })?
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
            .call_async(GetSnapshotMessage, self.timeout)
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
        self.actor_ref
            .call_async(ShutdownMessage, self.timeout)
            .await
            .map_err(|error| Self::map_call_error("shutdown", self.timeout, error))?
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
