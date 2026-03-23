use stellatune_audio_core::pipeline::context::{InputRef, PipelineContext, SourceHandle};
use stellatune_audio_core::pipeline::error::PipelineError;
use stellatune_audio_core::pipeline::stages::Stage;
use stellatune_audio_core::pipeline::stages::source::SourceStage;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginSourcePayload {
    pub track_token: String,
}

pub struct PluginSourceStage;

impl PluginSourceStage {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PluginSourceStage {
    fn default() -> Self {
        Self::new()
    }
}

impl Stage for PluginSourceStage {}

impl SourceStage for PluginSourceStage {
    fn prepare(
        &mut self,
        input: &InputRef,
        _ctx: &mut PipelineContext,
    ) -> Result<SourceHandle, PipelineError> {
        let InputRef::TrackToken(track_token) = input;
        let track_token = track_token.trim();
        if track_token.is_empty() {
            return Err(PipelineError::StageFailure(
                "track token must not be empty".to_string(),
            ));
        }
        Ok(SourceHandle::new(PluginSourcePayload {
            track_token: track_token.to_string(),
        }))
    }

    fn stop(&mut self, _ctx: &mut PipelineContext) {}
}

pub fn build_plugin_source() -> Box<dyn SourceStage> {
    Box::new(PluginSourceStage::new())
}

pub fn plugin_track_token_from_source_handle(source: &SourceHandle) -> Option<&str> {
    source
        .downcast_ref::<PluginSourcePayload>()
        .map(|v| v.track_token.as_str())
}
