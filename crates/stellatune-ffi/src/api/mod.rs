use std::thread;

use crate::frb_generated::{RustOpaque, StreamSink};
use anyhow::Result;

use stellatune_audio::start_engine;
use stellatune_core::{Command, Event};

pub struct Player {
    engine: stellatune_audio::EngineHandle,
}

impl Player {
    fn new() -> Self {
        Self {
            engine: start_engine(),
        }
    }
}

pub fn create_player() -> RustOpaque<Player> {
    RustOpaque::new(Player::new())
}

pub fn load(player: RustOpaque<Player>, path: String) {
    player.engine.send_command(Command::LoadTrack { path });
}

pub fn play(player: RustOpaque<Player>) {
    player.engine.send_command(Command::Play);
}

pub fn pause(player: RustOpaque<Player>) {
    player.engine.send_command(Command::Pause);
}

pub fn stop(player: RustOpaque<Player>) {
    player.engine.send_command(Command::Stop);
}

pub fn events(player: RustOpaque<Player>, sink: StreamSink<Event>) -> Result<()> {
    let rx = player.engine.subscribe_events();

    thread::Builder::new()
        .name("stellatune-events".to_string())
        .spawn(move || {
            for event in rx.iter() {
                if sink.add(event).is_err() {
                    break;
                }
            }
        })
        .expect("failed to spawn stellatune-events thread");

    Ok(())
}
