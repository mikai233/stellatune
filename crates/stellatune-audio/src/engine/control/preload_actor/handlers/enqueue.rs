use std::time::Instant;

use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};

use crate::engine::decode::decoder::open_engine_decoder;
use crate::engine::messages::{InternalMsg, PredecodedChunk};

use super::super::PreloadActor;

pub(crate) struct PreloadEnqueueMessage {
    pub(crate) path: String,
    pub(crate) position_ms: u64,
    pub(crate) token: u64,
}

impl Message for PreloadEnqueueMessage {
    type Response = ();
}

impl Handler<PreloadEnqueueMessage> for PreloadActor {
    fn handle(&mut self, message: PreloadEnqueueMessage, _ctx: &mut ActorContext<Self>) {
        let internal_tx = self.internal_tx.clone();
        let path = message.path;
        let position_ms = message.position_ms;
        let token = message.token;

        let t0 = Instant::now();
        match open_engine_decoder(&path) {
            Ok((mut decoder, track_info)) => {
                if position_ms > 0
                    && let Err(err) = decoder.seek_ms(position_ms)
                {
                    let _ = internal_tx.send(InternalMsg::PreloadFailed {
                        path: path.clone(),
                        position_ms,
                        message: err,
                        took_ms: t0.elapsed().as_millis() as u64,
                        token,
                    });
                    return;
                }
                match decoder.next_block(2048) {
                    Ok(Some(samples)) if !samples.is_empty() => {
                        let _ = internal_tx.send(InternalMsg::PreloadReady {
                            path: path.clone(),
                            position_ms,
                            track_info: track_info.clone(),
                            chunk: PredecodedChunk {
                                samples,
                                sample_rate: track_info.sample_rate,
                                channels: track_info.channels,
                                start_at_ms: position_ms,
                            },
                            took_ms: t0.elapsed().as_millis() as u64,
                            token,
                        });
                    }
                    Ok(_) => {
                        let _ = internal_tx.send(InternalMsg::PreloadFailed {
                            path: path.clone(),
                            position_ms,
                            message: "decoder returned no preload audio".to_string(),
                            took_ms: t0.elapsed().as_millis() as u64,
                            token,
                        });
                    }
                    Err(err) => {
                        let _ = internal_tx.send(InternalMsg::PreloadFailed {
                            path: path.clone(),
                            position_ms,
                            message: err,
                            took_ms: t0.elapsed().as_millis() as u64,
                            token,
                        });
                    }
                }
            }
            Err(err) => {
                let _ = internal_tx.send(InternalMsg::PreloadFailed {
                    path: path.clone(),
                    position_ms,
                    message: err,
                    took_ms: t0.elapsed().as_millis() as u64,
                    token,
                });
            }
        }
    }
}
