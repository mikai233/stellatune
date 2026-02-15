use stellatune_runtime::tokio_actor::{ActorContext, Handler, Message};

use crate::engine::control::runtime_query::output_sink_owner_actor::OutputSinkOwnerActor;
use crate::engine::control::runtime_query::{
    apply_or_recreate_output_sink_instance, create_output_sink_cached_instance,
    with_runtime_service,
};

pub(crate) struct OutputSinkListTargetsMessage {
    pub config_json: String,
}

impl Message for OutputSinkListTargetsMessage {
    type Response = Result<String, String>;
}

#[async_trait::async_trait]
impl Handler<OutputSinkListTargetsMessage> for OutputSinkOwnerActor {
    async fn handle(
        &mut self,
        message: OutputSinkListTargetsMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<String, String> {
        if self.frozen {
            return Err(format!(
                "output sink owner frozen for {}::{}",
                self.slot.plugin_id, self.slot.type_id
            ));
        }

        if self.entry.is_none() {
            let created = with_runtime_service(|service| {
                create_output_sink_cached_instance(
                    service,
                    &self.slot.plugin_id,
                    &self.slot.type_id,
                    &message.config_json,
                )
            })?;
            self.entry = Some(created);
        }

        let plugin_id = self.slot.plugin_id.clone();
        let type_id = self.slot.type_id.clone();
        let entry = self
            .entry
            .as_mut()
            .ok_or_else(|| "output sink owner cache insertion failed".to_string())?;
        apply_or_recreate_output_sink_instance(&plugin_id, &type_id, entry, &message.config_json)?;
        let instance = entry.controller.instance_mut().ok_or_else(|| {
            format!("output sink instance unavailable for {plugin_id}::{type_id}")
        })?;
        instance.list_targets_json().map_err(|e| e.to_string())
    }
}
