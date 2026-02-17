use std::sync::Arc;

use tokio::sync::broadcast;

use crate::config::engine::{EngineSnapshot, Event};
use crate::engine::handle::EngineHandle;
use crate::engine::messages::{
    ApplyPipelineMutationMessage, ApplyPipelinePlanMessage, GetSnapshotMessage, ShutdownMessage,
};
use crate::error::EngineError;
use crate::pipeline::assembly::{PipelineMutation, PipelinePlan};

impl EngineHandle {
    pub async fn apply_pipeline_plan(
        &self,
        plan: Arc<dyn PipelinePlan>,
    ) -> Result<(), EngineError> {
        self.actor_ref
            .call_async(ApplyPipelinePlanMessage { plan }, self.timeout)
            .await
            .map_err(|error| Self::map_call_error("apply_pipeline_plan", self.timeout, error))?
    }

    pub async fn apply_pipeline_mutation(
        &self,
        mutation: PipelineMutation,
    ) -> Result<(), EngineError> {
        self.actor_ref
            .call_async(ApplyPipelineMutationMessage { mutation }, self.timeout)
            .await
            .map_err(|error| Self::map_call_error("apply_pipeline_mutation", self.timeout, error))?
    }

    pub async fn snapshot(&self) -> Result<EngineSnapshot, EngineError> {
        self.actor_ref
            .call_async(GetSnapshotMessage, self.timeout)
            .await
            .map_err(|error| Self::map_call_error("snapshot", self.timeout, error))
    }

    pub async fn shutdown(&self) -> Result<(), EngineError> {
        self.actor_ref
            .call_async(ShutdownMessage, self.timeout)
            .await
            .map_err(|error| Self::map_call_error("shutdown", self.timeout, error))?
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<Event> {
        self.events.subscribe()
    }
}
