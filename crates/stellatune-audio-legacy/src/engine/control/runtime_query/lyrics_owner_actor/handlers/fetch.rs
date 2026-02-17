use stellatune_runtime::tokio_actor::{ActorContext, Handler, Message};

use crate::engine::control::runtime_query::lyrics_owner_actor::LyricsOwnerActor;
use crate::engine::control::runtime_query::{
    apply_or_recreate_lyrics_instance, create_lyrics_provider_cached_instance, with_runtime_service,
};

pub(crate) struct LyricsFetchMessage {
    pub config_json: String,
    pub track_json: String,
}

impl Message for LyricsFetchMessage {
    type Response = Result<String, String>;
}

#[async_trait::async_trait]
impl Handler<LyricsFetchMessage> for LyricsOwnerActor {
    async fn handle(
        &mut self,
        message: LyricsFetchMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<String, String> {
        if self.frozen {
            return Err(format!(
                "lyrics owner frozen for {}::{}",
                self.slot.plugin_id, self.slot.type_id
            ));
        }

        if self.entry.is_none() {
            let created = with_runtime_service(|service| {
                create_lyrics_provider_cached_instance(
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
            .ok_or_else(|| "lyrics owner task cache insertion failed".to_string())?;
        apply_or_recreate_lyrics_instance(&plugin_id, &type_id, entry, &message.config_json)?;
        let instance = entry
            .controller
            .instance_mut()
            .ok_or_else(|| format!("lyrics instance unavailable for {}::{}", plugin_id, type_id))?;
        instance
            .fetch_json(&message.track_json)
            .map_err(|e| e.to_string())
    }
}
