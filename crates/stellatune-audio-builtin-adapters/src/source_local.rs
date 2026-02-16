use stellatune_audio_core::pipeline::context::{InputRef, PipelineContext, SourceHandle};
use stellatune_audio_core::pipeline::error::PipelineError;
use stellatune_audio_core::pipeline::stages::source::SourceStage;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalSourcePayload {
    pub track_token: String,
}

pub struct LocalSourceStage {
    payload: LocalSourcePayload,
}

impl LocalSourceStage {
    pub fn new(track_token: String) -> Self {
        Self {
            payload: LocalSourcePayload { track_token },
        }
    }
}

impl SourceStage for LocalSourceStage {
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

pub fn build_local_source(track_token: String) -> Box<dyn SourceStage> {
    Box::new(LocalSourceStage::new(track_token))
}

pub fn local_track_token_from_source_handle(source: &SourceHandle) -> Option<&str> {
    source
        .downcast_ref::<LocalSourcePayload>()
        .map(|v| v.track_token.as_str())
}
