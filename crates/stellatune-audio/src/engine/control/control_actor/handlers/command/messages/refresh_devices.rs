use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};

use crate::engine::control::control_actor::ControlActor;
use crate::engine::control::output_backend_for_selected;

pub(crate) struct RefreshDevicesMessage;

impl Message for RefreshDevicesMessage {
    type Response = Result<Vec<crate::types::AudioDevice>, String>;
}

impl Handler<RefreshDevicesMessage> for ControlActor {
    fn handle(
        &mut self,
        _message: RefreshDevicesMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<Vec<crate::types::AudioDevice>, String> {
        let selected_backend = output_backend_for_selected(self.state.selected_backend);
        let devices: Vec<crate::types::AudioDevice> =
            stellatune_output::list_host_devices(Some(selected_backend))
                .into_iter()
                .map(|d| crate::types::AudioDevice {
                    backend: match d.backend {
                        stellatune_output::AudioBackend::Shared => {
                            crate::types::AudioBackend::Shared
                        },
                        stellatune_output::AudioBackend::WasapiExclusive => {
                            crate::types::AudioBackend::WasapiExclusive
                        },
                    },
                    id: d.id,
                    name: d.name,
                })
                .collect();
        Ok(devices)
    }
}
