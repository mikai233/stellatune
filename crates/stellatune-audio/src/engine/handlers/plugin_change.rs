use lattice_actor::{
    context::HandlerContext, error::ActorError, reply::ReplyTo, traits::Responder,
};

use crate::config::engine::PlaybackState;
use crate::engine::actor::PlaybackActor;
use crate::engine::messages::{
    AbortPluginChangeMessage, CompletePluginChangeMessage, SuspendForPluginChangeMessage,
};
use crate::error::EngineError;
use crate::pipeline::plan::PlaybackCheckpoint;

impl Responder<SuspendForPluginChangeMessage> for PlaybackActor {
    async fn respond(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        _message: SuspendForPluginChangeMessage,
        reply_to: ReplyTo<Result<Option<PlaybackCheckpoint>, EngineError>>,
    ) -> Result<(), ActorError> {
        let resume_playing = *ctx.behavior() == PlaybackState::Playing;
        self.pump_scheduled = false;
        self.transition_state(ctx, PlaybackState::Reconfiguring);
        let result = self.ensure_session().and_then(|session| {
            session
                .suspend_for_plugin_change(resume_playing)
                .map_err(EngineError::from)
        });
        match result {
            Ok(checkpoint) => {
                self.plugin_checkpoint = checkpoint.clone();
                if let Some(checkpoint) = checkpoint.as_ref() {
                    self.position_ms = checkpoint.consumed_position_ms;
                }
                let _ = reply_to.send(Ok(checkpoint));
            },
            Err(error) => {
                self.transition_state(ctx, PlaybackState::Idle);
                let _ = reply_to.send(Err(error));
            },
        }
        Ok(())
    }
}

impl Responder<CompletePluginChangeMessage> for PlaybackActor {
    async fn respond(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        _message: CompletePluginChangeMessage,
        reply_to: ReplyTo<Result<(), EngineError>>,
    ) -> Result<(), ActorError> {
        let checkpoint = self.plugin_checkpoint.take();
        let result = match checkpoint.as_ref() {
            Some(checkpoint) => self.ensure_session().and_then(|session| {
                session
                    .restore_after_plugin_change(checkpoint)
                    .map_err(EngineError::from)
            }),
            None => Ok(()),
        };
        if result.is_ok() {
            let next = match checkpoint {
                Some(checkpoint) if checkpoint.resume_playing => PlaybackState::Playing,
                Some(_) => PlaybackState::Ready,
                None => PlaybackState::Idle,
            };
            self.transition_state(ctx, next);
            if next == PlaybackState::Playing {
                self.schedule_pump(ctx, std::time::Duration::ZERO);
            }
        } else {
            self.transition_state(ctx, PlaybackState::Idle);
        }
        let _ = reply_to.send(result);
        Ok(())
    }
}

impl Responder<AbortPluginChangeMessage> for PlaybackActor {
    async fn respond(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        _message: AbortPluginChangeMessage,
        reply_to: ReplyTo<Result<(), EngineError>>,
    ) -> Result<(), ActorError> {
        <Self as Responder<CompletePluginChangeMessage>>::respond(
            self,
            ctx,
            CompletePluginChangeMessage,
            reply_to,
        )
        .await
    }
}
