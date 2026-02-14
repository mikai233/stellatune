use stellatune_runtime::tokio_actor::{ActorContext, Handler, Message};

use super::super::SourceOwnerActor;
use crate::engine::control::runtime_query::RuntimeSourceStreamLease;

pub(crate) struct SourceOpenStreamMessage {
    pub config_json: String,
    pub track_json: String,
    pub stream_id: u64,
}

impl Message for SourceOpenStreamMessage {
    type Response = Result<RuntimeSourceStreamLease, String>;
}

#[async_trait::async_trait]
impl Handler<SourceOpenStreamMessage> for SourceOwnerActor {
    async fn handle(
        &mut self,
        message: SourceOpenStreamMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<RuntimeSourceStreamLease, String> {
        let plugin_id = self.slot.plugin_id.clone();
        let type_id = self.slot.type_id.clone();
        match self.ensure_current_entry_for_ops(&message.config_json) {
            Ok(current) => {
                let lease_id = current.lease_id;
                let instance = current.entry.controller.instance_mut().ok_or_else(|| {
                    format!("source instance unavailable for {}::{}", plugin_id, type_id)
                });
                match instance {
                    Ok(instance) => {
                        let opened = instance
                            .open_stream(&message.track_json)
                            .await
                            .map_err(|e| e.to_string());
                        match opened {
                            Ok((stream, source_metadata_json)) => {
                                let io_vtable_addr = stream.io_vtable as usize;
                                let io_handle_addr = stream.io_handle as usize;
                                if io_vtable_addr == 0 || io_handle_addr == 0 {
                                    if io_handle_addr != 0 {
                                        instance.close_stream(stream.io_handle);
                                    }
                                    Err("source open_stream returned null io_vtable/io_handle"
                                        .to_string())
                                } else {
                                    self.streams.insert(
                                        message.stream_id,
                                        super::super::SourceStreamLeaseRecord {
                                            lease_id,
                                            io_handle_addr,
                                        },
                                    );
                                    Ok(RuntimeSourceStreamLease {
                                        stream_id: message.stream_id,
                                        lease_id,
                                        io_vtable_addr,
                                        io_handle_addr,
                                        source_metadata_json,
                                    })
                                }
                            }
                            Err(err) => Err(err),
                        }
                    }
                    Err(err) => Err(err),
                }
            }
            Err(err) => Err(err),
        }
    }
}
