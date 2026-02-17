use std::any::Any;

use crate::config::engine::{Event, LfeMode, ResampleQuality};
use crate::engine::handle::EngineHandle;
use crate::engine::messages::{
    ApplyStageControlMessage, SetLfeModeMessage, SetResampleQualityMessage,
};
use crate::error::EngineError;

impl EngineHandle {
    /// Updates the hot master-gain target used by the runtime.
    ///
    /// The change is published immediately to subscribers as
    /// [`Event::VolumeChanged`].
    ///
    /// # Errors
    ///
    /// This method currently does not fail and always returns `Ok(())`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use stellatune_audio::engine::EngineHandle;
    ///
    /// # async fn demo(handle: &EngineHandle) -> Result<(), stellatune_audio::error::EngineError> {
    /// handle.set_volume(0.8, 42, 24).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn set_volume(&self, volume: f32, seq: u64, ramp_ms: u32) -> Result<(), EngineError> {
        self.master_gain_hot_control.update(volume, ramp_ms, None);
        self.events.emit(Event::VolumeChanged { volume, seq });
        Ok(())
    }

    /// Sets the mixer LFE routing mode.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError`] when the control actor call fails or the decode
    /// worker rejects the mode transition.
    pub async fn set_lfe_mode(&self, mode: LfeMode) -> Result<(), EngineError> {
        self.actor_ref
            .call_async(SetLfeModeMessage { mode }, self.timeout)
            .await
            .map_err(|error| Self::map_call_error("set_lfe_mode", self.timeout, error))?
    }

    /// Sets the resampler quality policy used by runtime rebuilds.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError`] when the control actor call fails or the decode
    /// worker cannot apply the quality update.
    pub async fn set_resample_quality(&self, quality: ResampleQuality) -> Result<(), EngineError> {
        self.actor_ref
            .call_async(SetResampleQualityMessage { quality }, self.timeout)
            .await
            .map_err(|error| Self::map_call_error("set_resample_quality", self.timeout, error))?
    }

    /// Applies a typed control payload to a transform stage by key.
    ///
    /// The payload type must match what the target stage expects at runtime.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError`] when the control actor call fails, the stage key
    /// does not exist, or the payload type is unsupported for that stage.
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
