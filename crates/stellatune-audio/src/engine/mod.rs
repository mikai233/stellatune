mod actor;
mod handle;
mod handlers;
mod messages;
mod startup;

pub type EngineHandle = handle::EngineHandle;

pub fn start_engine(
    assembler: std::sync::Arc<dyn crate::pipeline::assembly::PipelineAssembler>,
) -> Result<EngineHandle, crate::error::EngineError> {
    startup::start_engine(assembler)
}

pub fn start_engine_with_config(
    assembler: std::sync::Arc<dyn crate::pipeline::assembly::PipelineAssembler>,
    config: crate::config::engine::EngineConfig,
) -> Result<EngineHandle, crate::error::EngineError> {
    startup::start_engine_with_config(assembler, config)
}
