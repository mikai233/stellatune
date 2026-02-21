mod decoder;
mod io;

use stellatune_plugin_sdk::prelude::*;

use crate::decoder::StreamSymphoniaDecoderSession;

pub struct NeteaseStreamDecoderPlugin;

impl PluginLifecycle for NeteaseStreamDecoderPlugin {}

impl DecoderPlugin for NeteaseStreamDecoderPlugin {
    type Session = StreamSymphoniaDecoderSession;

    const TYPE_ID: &'static str = "stream_symphonia";
    const DISPLAY_NAME: &'static str = "Stream Decoder (Symphonia)";

    fn open(&mut self, input: DecoderInput) -> SdkResult<Self::Session> {
        StreamSymphoniaDecoderSession::open(input)
    }
}

fn create_plugin() -> SdkResult<NeteaseStreamDecoderPlugin> {
    Ok(NeteaseStreamDecoderPlugin)
}

stellatune_plugin_sdk::export_decoder_component! {
    plugin_type: crate::NeteaseStreamDecoderPlugin,
    create: crate::create_plugin,
}
