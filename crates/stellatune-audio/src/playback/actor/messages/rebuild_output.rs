//! Negotiates an output rebuild off-turn so device probing cannot block control.
use super::super::PlaybackActor;
use super::{ControlResult, output_prepared::OutputPrepared};
use crate::playback::{normalizer::PcmNormalizer, pipeline::ConfiguredTransform};
use lattice_actor::{
    context::HandlerContext, error::ActorError, reply::ReplyTo, traits::Responder,
};
use std::sync::Arc;
use stellatune_audio_core::{
    error::{FailureStage, PlaybackControlError},
    format::PcmFormat,
    sink::SinkFactory,
    transform::{TransformFactory, TransformPlacement},
};

pub(in crate::playback::actor) struct PreparedOutput {
    pub(in crate::playback::actor) target: PcmFormat,
    pub(in crate::playback::actor) normalizer: PcmNormalizer,
    pub(in crate::playback::actor) output_format: PcmFormat,
    pub(in crate::playback::actor) transforms: Vec<ConfiguredTransform>,
}

#[derive(lattice_actor::Request)]
#[request(response = ControlResult)]
pub(in crate::playback) struct RebuildOutput;

impl Responder<RebuildOutput> for PlaybackActor {
    async fn respond(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        _request: RebuildOutput,
        reply_to: ReplyTo<ControlResult>,
    ) -> Result<(), ActorError> {
        let Some(current) = self.session.current.as_ref() else {
            let _ = reply_to.send(Ok(()));
            return Ok(());
        };
        if !current
            .recovery_plan
            .item
            .source
            .descriptor()
            .capabilities
            .byte_seekable
        {
            let _ = reply_to.send(Err(PlaybackControlError::Unsupported));
            return Ok(());
        }
        self.session.output_rebuild_id = self.session.output_rebuild_id.wrapping_add(1);
        let id = self.session.output_rebuild_id;
        let generation = self.session.generation;
        let item_id = current.item_id;
        let sink = Arc::clone(&current.sink_factory);
        let transforms = current.recovery_plan.transforms.clone();
        let input = current.pipeline.normalizer_input_format;
        let timeout = self.config.command_timeouts.output_rebuild;
        let _ = ctx.defer_reply(
            reply_to,
            async move {
                tokio::time::timeout(
                    timeout,
                    tokio::task::spawn_blocking(move || negotiate(sink, transforms, input)),
                )
                .await
                .map_err(|_| PlaybackControlError::CommandTimeout {
                    operation: "rebuild_output",
                })?
                .map_err(|error| {
                    PlaybackControlError::failed(FailureStage::Sink, error.to_string())
                })?
            },
            move |result, reply_to| OutputPrepared {
                id,
                generation,
                item_id,
                result,
                reply_to,
            },
        );
        Ok(())
    }
}

fn negotiate(
    sink: Arc<dyn SinkFactory>,
    factories: Vec<Arc<dyn TransformFactory>>,
    input: PcmFormat,
) -> Result<PreparedOutput, PlaybackControlError> {
    let target = sink.preferred_format(input).map_err(|error| {
        PlaybackControlError::factory(FailureStage::Sink, sink.id().clone(), error)
    })?;
    target
        .validate()
        .map_err(|message| PlaybackControlError::failed(FailureStage::Sink, message))?;
    let normalizer = PcmNormalizer::new(input, target)?;
    let mut output_format = target;
    let mut transforms = Vec::new();
    for factory in factories {
        if factory.descriptor().placement != TransformPlacement::PostMix {
            continue;
        }
        let mut stage = factory.create().map_err(|error| {
            PlaybackControlError::factory(
                FailureStage::Transform,
                factory.descriptor().id.clone(),
                error,
            )
        })?;
        output_format = stage.configure(output_format).map_err(|error| {
            PlaybackControlError::transform(error, factory.descriptor().id.clone())
        })?;
        transforms.push(ConfiguredTransform::new(
            stage,
            output_format,
            factory.descriptor().id.clone(),
        ));
    }
    Ok(PreparedOutput {
        target,
        normalizer,
        output_format,
        transforms,
    })
}
