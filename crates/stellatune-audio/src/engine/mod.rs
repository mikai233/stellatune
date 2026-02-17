//! Engine startup and control-handle entry points.
//!
//! This module exposes the constructors used to bootstrap the runtime engine and
//! the public [`EngineHandle`] type used to issue control operations.

mod actor;
mod handle;
mod handlers;
mod messages;
mod startup;

/// Handle type for interacting with the running audio engine.
///
/// See [`EngineHandle`] for the async operation surface.
pub type EngineHandle = handle::EngineHandle;

/// Starts the audio engine with default runtime configuration.
///
/// The returned [`EngineHandle`] owns communication channels to the control actor
/// and decode worker.
///
/// # Errors
///
/// Returns [`crate::error::EngineError`] when the control actor cannot be
/// spawned.
///
/// # Examples
///
/// ```no_run
/// use std::sync::Arc;
///
/// use stellatune_audio::engine::start_engine;
/// use stellatune_audio::pipeline::assembly::PipelineAssembler;
///
/// # fn assembler() -> Arc<dyn PipelineAssembler> { todo!() }
/// # async fn boot() -> Result<(), stellatune_audio::error::EngineError> {
/// let engine = start_engine(assembler())?;
/// let _ = engine.snapshot().await?;
/// # Ok(())
/// # }
/// ```
pub fn start_engine(
    assembler: std::sync::Arc<dyn crate::pipeline::assembly::PipelineAssembler>,
) -> Result<EngineHandle, crate::error::EngineError> {
    startup::start_engine(assembler)
}

/// Starts the audio engine with an explicit [`crate::config::engine::EngineConfig`].
///
/// This is equivalent to [`start_engine`] but allows callers to override command
/// timeouts, worker queue sizes, and sink-related runtime policy.
///
/// # Errors
///
/// Returns [`crate::error::EngineError`] when the control actor cannot be
/// spawned.
///
/// # Examples
///
/// ```no_run
/// use std::sync::Arc;
///
/// use stellatune_audio::config::engine::EngineConfig;
/// use stellatune_audio::engine::start_engine_with_config;
/// use stellatune_audio::pipeline::assembly::PipelineAssembler;
///
/// # fn assembler() -> Arc<dyn PipelineAssembler> { todo!() }
/// # fn config() -> EngineConfig { EngineConfig::default() }
/// # fn boot() -> Result<(), stellatune_audio::error::EngineError> {
/// let _engine = start_engine_with_config(assembler(), config())?;
/// # Ok(())
/// # }
/// ```
pub fn start_engine_with_config(
    assembler: std::sync::Arc<dyn crate::pipeline::assembly::PipelineAssembler>,
    config: crate::config::engine::EngineConfig,
) -> Result<EngineHandle, crate::error::EngineError> {
    startup::start_engine_with_config(assembler, config)
}
