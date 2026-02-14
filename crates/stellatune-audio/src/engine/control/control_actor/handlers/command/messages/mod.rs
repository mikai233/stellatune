use std::sync::Arc;

use crate::engine::control::Event;
use crate::engine::event_hub::EventHub;

pub(crate) mod clear_output_sink_route;
pub(crate) mod pause;
pub(crate) mod play;
pub(crate) mod preload_track;
pub(crate) mod preload_track_ref;
pub(crate) mod refresh_devices;
pub(crate) mod seek_ms;
pub(crate) mod set_output_device;
pub(crate) mod set_output_options;
pub(crate) mod set_output_sink_route;
pub(crate) mod set_volume;
pub(crate) mod shutdown;
pub(crate) mod stop;
pub(crate) mod switch_track_ref;

pub(crate) use clear_output_sink_route::ClearOutputSinkRouteMessage;
pub(crate) use pause::PauseMessage;
pub(crate) use play::PlayMessage;
pub(crate) use preload_track::PreloadTrackMessage;
pub(crate) use preload_track_ref::PreloadTrackRefMessage;
pub(crate) use refresh_devices::RefreshDevicesMessage;
pub(crate) use seek_ms::SeekMsMessage;
pub(crate) use set_output_device::SetOutputDeviceMessage;
pub(crate) use set_output_options::SetOutputOptionsMessage;
pub(crate) use set_output_sink_route::SetOutputSinkRouteMessage;
pub(crate) use set_volume::SetVolumeMessage;
pub(crate) use shutdown::ShutdownMessage;
pub(crate) use stop::StopMessage;
pub(crate) use switch_track_ref::SwitchTrackRefMessage;

pub(super) fn emit_and_err<T>(
    events: &Arc<EventHub>,
    message: impl Into<String>,
) -> Result<T, String> {
    let message = message.into();
    events.emit(Event::Error {
        message: message.clone(),
    });
    Err(message)
}
