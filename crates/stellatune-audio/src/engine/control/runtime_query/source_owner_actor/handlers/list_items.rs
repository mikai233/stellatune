use stellatune_runtime::tokio_actor::{ActorContext, Handler, Message};

use super::super::SourceOwnerActor;

pub(crate) struct SourceListItemsMessage {
    pub config_json: String,
    pub request_json: String,
}

impl Message for SourceListItemsMessage {
    type Response = Result<String, String>;
}

#[async_trait::async_trait]
impl Handler<SourceListItemsMessage> for SourceOwnerActor {
    async fn handle(
        &mut self,
        message: SourceListItemsMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<String, String> {
        let plugin_id = self.slot.plugin_id.clone();
        let type_id = self.slot.type_id.clone();
        match self.ensure_current_entry_for_ops(&message.config_json) {
            Ok(current) => {
                let instance = current.entry.controller.instance_mut().ok_or_else(|| {
                    format!("source instance unavailable for {}::{}", plugin_id, type_id)
                });
                match instance {
                    Ok(instance) => instance
                        .list_items_json(&message.request_json)
                        .await
                        .map_err(|e| e.to_string()),
                    Err(err) => Err(err),
                }
            }
            Err(err) => Err(err),
        }
    }
}
