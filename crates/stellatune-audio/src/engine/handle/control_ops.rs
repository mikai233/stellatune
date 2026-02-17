use std::any::Any;

use crate::config::engine::{Event, LfeMode, ResampleQuality};
use crate::engine::handle::EngineHandle;
use crate::engine::messages::{
    ApplyStageControlMessage, SetLfeModeMessage, SetResampleQualityMessage,
};
use crate::error::EngineError;

impl EngineHandle {
    pub async fn set_volume(&self, volume: f32, seq: u64, ramp_ms: u32) -> Result<(), EngineError> {
        self.master_gain_hot_control.update(volume, ramp_ms, None);
        self.events.emit(Event::VolumeChanged { volume, seq });
        Ok(())
    }

    pub async fn set_lfe_mode(&self, mode: LfeMode) -> Result<(), EngineError> {
        self.actor_ref
            .call_async(SetLfeModeMessage { mode }, self.timeout)
            .await
            .map_err(|error| Self::map_call_error("set_lfe_mode", self.timeout, error))?
    }

    pub async fn set_resample_quality(&self, quality: ResampleQuality) -> Result<(), EngineError> {
        self.actor_ref
            .call_async(SetResampleQualityMessage { quality }, self.timeout)
            .await
            .map_err(|error| Self::map_call_error("set_resample_quality", self.timeout, error))?
    }

    pub async fn apply_stage_control<T>(
        &self,
        stage_key: impl Into<String>,
        control: T,
    ) -> Result<(), EngineError>
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
            .map_err(|error| Self::map_call_error("apply_stage_control", self.timeout, error))?
    }
}
