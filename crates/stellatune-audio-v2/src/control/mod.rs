mod actor;
mod engine_handle;
mod handlers;
mod messages;
mod startup;

pub type EngineHandle = engine_handle::EngineHandle;

pub fn start_engine(
    assembler: std::sync::Arc<dyn crate::assembly::PipelineAssembler>,
) -> Result<EngineHandle, String> {
    startup::start_engine(assembler)
}

pub fn start_engine_with_config(
    assembler: std::sync::Arc<dyn crate::assembly::PipelineAssembler>,
    config: crate::types::EngineConfig,
) -> Result<EngineHandle, String> {
    startup::start_engine_with_config(assembler, config)
}
