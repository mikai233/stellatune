use stellatune_audio_core::pipeline::context::{InputRef, PipelineContext, SourceHandle};
use stellatune_audio_core::pipeline::error::PipelineError;
use stellatune_audio_core::pipeline::stages::source::SourceStage;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginSourcePayload {
    pub track_token: String,
}

pub struct PluginSourceStage {
    payload: PluginSourcePayload,
}

impl PluginSourceStage {
    pub fn new(track_token: String) -> Self {
        Self {
            payload: PluginSourcePayload { track_token },
        }
    }
}

impl SourceStage for PluginSourceStage {
    fn prepare(
        &mut self,
        _input: &InputRef,
        _ctx: &mut PipelineContext,
    ) -> Result<SourceHandle, PipelineError> {
        Ok(SourceHandle::new(self.payload.clone()))
    }

    fn sync_runtime_control(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
        Ok(())
    }

    fn stop(&mut self, _ctx: &mut PipelineContext) {}
}

pub fn build_plugin_source(track_token: String) -> Box<dyn SourceStage> {
    Box::new(PluginSourceStage::new(track_token))
}

pub fn plugin_track_token_from_source_handle(source: &SourceHandle) -> Option<&str> {
    source
        .downcast_ref::<PluginSourcePayload>()
        .map(|v| v.track_token.as_str())
}
