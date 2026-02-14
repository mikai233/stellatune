use std::sync::Arc;

use crossbeam_channel::Sender;
use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};

use super::super::super::{EngineState, SharedTrackInfo};
use super::super::ControlActor;
use crate::engine::event_hub::EventHub;
use crate::engine::messages::InternalMsg;

pub(super) mod errors;
pub(super) mod output_spec;
pub(super) mod preload;

pub(super) struct InternalCtx<'a> {
    pub(super) state: &'a mut EngineState,
    pub(super) events: &'a Arc<EventHub>,
    pub(super) internal_tx: &'a Sender<InternalMsg>,
    pub(super) track_info: &'a SharedTrackInfo,
}

pub(crate) struct ControlInternalMessage {
    pub(crate) message: InternalMsg,
}

impl Message for ControlInternalMessage {
    type Response = ();
}

impl Handler<ControlInternalMessage> for ControlActor {
    fn handle(&mut self, message: ControlInternalMessage, _ctx: &mut ActorContext<Self>) {
        let mut internal_ctx = InternalCtx {
            state: &mut self.state,
            events: &self.events,
            internal_tx: &self.internal_tx,
            track_info: &self.track_info,
        };

        match message.message {
            InternalMsg::Eof => errors::on_eof(&mut internal_ctx),
            InternalMsg::Error(message) => errors::on_error(&mut internal_ctx, message),
            InternalMsg::OutputError(message) => {
                errors::on_output_error(&mut internal_ctx, message)
            }
            InternalMsg::Position { path, ms } => errors::on_position(&mut internal_ctx, path, ms),
            InternalMsg::OutputSpecReady {
                spec,
                took_ms,
                token,
            } => output_spec::on_output_spec_ready(&mut internal_ctx, spec, took_ms, token),
            InternalMsg::OutputSpecFailed {
                message,
                took_ms,
                token,
            } => output_spec::on_output_spec_failed(&mut internal_ctx, message, took_ms, token),
            InternalMsg::PreloadReady {
                path,
                position_ms,
                track_info,
                chunk,
                took_ms,
                token,
            } => preload::on_preload_ready(
                &mut internal_ctx,
                preload::PreloadReadyArgs {
                    path,
                    position_ms,
                    track_info,
                    chunk,
                    took_ms,
                    token,
                },
            ),
            InternalMsg::PreloadFailed {
                path,
                position_ms,
                message,
                took_ms,
                token,
            } => preload::on_preload_failed(
                &mut internal_ctx,
                path,
                position_ms,
                message,
                took_ms,
                token,
            ),
        }
    }
}
