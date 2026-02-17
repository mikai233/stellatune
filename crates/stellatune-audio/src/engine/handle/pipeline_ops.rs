use std::sync::Arc;

use tokio::sync::broadcast;

use crate::config::engine::{EngineSnapshot, Event};
use crate::engine::handle::EngineHandle;
use crate::engine::messages::{
    ApplyPipelineMutationMessage, ApplyPipelinePlanMessage, GetSnapshotMessage, ShutdownMessage,
};
use crate::pipeline::assembly::{PipelineMutation, PipelinePlan};

impl EngineHandle {
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
