use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};

use super::super::output_backend_for_selected;
use crate::engine::control::control_actor::ControlActor;

pub(crate) struct RefreshDevicesMessage;

impl Message for RefreshDevicesMessage {
    type Response = Result<Vec<stellatune_core::AudioDevice>, String>;
}

impl Handler<RefreshDevicesMessage> for ControlActor {
    fn handle(
        &mut self,
        _message: RefreshDevicesMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<Vec<stellatune_core::AudioDevice>, String> {
        let selected_backend = output_backend_for_selected(self.state.selected_backend);
        let devices: Vec<stellatune_core::AudioDevice> =
            stellatune_output::list_host_devices(Some(selected_backend))
                .into_iter()
                .map(|d| stellatune_core::AudioDevice {
                    backend: match d.backend {
                        stellatune_output::AudioBackend::Shared => {
                            stellatune_core::AudioBackend::Shared
                        }
                        stellatune_output::AudioBackend::WasapiExclusive => {
                            stellatune_core::AudioBackend::WasapiExclusive
                        }
                    },
                    id: d.id,
                    name: d.name,
                })
                .collect();
        Ok(devices)
    }
}
