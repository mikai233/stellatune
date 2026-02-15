use stellatune_runtime::tokio_actor::{ActorContext, Handler, Message};

use crate::engine::control::runtime_query::source_owner_actor::SourceOwnerActor;

pub(crate) struct SourceCloseStreamMessage {
    pub stream_id: u64,
}

impl Message for SourceCloseStreamMessage {
    type Response = Result<(), String>;
}

#[async_trait::async_trait]
impl Handler<SourceCloseStreamMessage> for SourceOwnerActor {
    async fn handle(
        &mut self,
        message: SourceCloseStreamMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<(), String> {
        let Some(record) = self.streams.remove(&message.stream_id) else {
            return Ok(());
        };

        if self
            .current
            .as_ref()
            .is_some_and(|c| c.lease_id == record.lease_id)
        {
            let current = self
                .current
                .as_mut()
                .ok_or_else(|| "source current lease missing while closing stream".to_string())?;
            let instance = current.entry.controller.instance_mut().ok_or_else(|| {
                format!(
                    "source instance unavailable while closing stream_id={}",
                    message.stream_id
                )
            })?;
            instance.close_stream(record.io_handle_addr as *mut core::ffi::c_void);
        } else if let Some(entry) = self.retired.get_mut(&record.lease_id) {
            let instance = entry.controller.instance_mut().ok_or_else(|| {
                format!(
                    "source retired instance unavailable while closing stream_id={} lease_id={}",
                    message.stream_id, record.lease_id
                )
            })?;
            instance.close_stream(record.io_handle_addr as *mut core::ffi::c_void);
        } else {
            return Err(format!(
                "source lease missing while closing stream_id={} lease_id={}",
                message.stream_id, record.lease_id
            ));
        }

        if self.active_streams_for_lease(record.lease_id) == 0 {
            self.retired.remove(&record.lease_id);
        }
        Ok(())
    }
}
