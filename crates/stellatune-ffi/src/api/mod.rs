use std::thread;

use crate::frb_generated::{RustOpaque, StreamSink};
use anyhow::Result;

use stellatune_audio::start_engine;
use stellatune_core::{Command, Event};

pub struct CoreService {
    engine: stellatune_audio::EngineHandle,
}

impl CoreService {
    fn new() -> Self {
        Self {
            engine: start_engine(),
        }
    }
}

pub fn create_core_service() -> RustOpaque<CoreService> {
    RustOpaque::new(CoreService::new())
}

pub fn send_command(service: RustOpaque<CoreService>, cmd: Command) {
    service.engine.send_command(cmd);
}

pub fn events_stream(service: RustOpaque<CoreService>, sink: StreamSink<Event>) -> Result<()> {
    let rx = service.engine.subscribe_events();

    thread::spawn(move || {
        for event in rx.iter() {
            if sink.add(event).is_err() {
                break;
            }
        }
    });

    Ok(())
}
