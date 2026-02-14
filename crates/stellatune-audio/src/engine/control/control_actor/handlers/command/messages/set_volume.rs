use std::sync::atomic::Ordering;

use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};

use super::super::ui_volume_to_gain;
use crate::engine::control::Event;
use crate::engine::control::control_actor::ControlActor;

pub(crate) struct SetVolumeMessage {
    pub(crate) volume: f32,
}

impl Message for SetVolumeMessage {
    type Response = Result<(), String>;
}

impl Handler<SetVolumeMessage> for ControlActor {
    fn handle(
        &mut self,
        message: SetVolumeMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<(), String> {
        let ui = message.volume.clamp(0.0, 1.0);
        let gain = ui_volume_to_gain(ui);
        self.state.volume = ui;
        self.state
            .volume_atomic
            .store(gain.to_bits(), Ordering::Relaxed);
        self.events.emit(Event::VolumeChanged { volume: ui });
        Ok(())
    }
}
