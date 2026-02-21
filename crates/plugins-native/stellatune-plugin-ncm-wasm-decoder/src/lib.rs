mod decoder;
mod flac_offset;
mod io;

use stellatune_wasm_plugin_sdk::prelude::*;

use crate::decoder::NcmWasmDecoderSession;

pub struct NcmWasmDecoderPlugin;

impl PluginLifecycle for NcmWasmDecoderPlugin {}

impl DecoderPlugin for NcmWasmDecoderPlugin {
    type Session = NcmWasmDecoderSession;

    const TYPE_ID: &'static str = "ncm-wasm";
    const DISPLAY_NAME: &'static str = "NCM Decoder (Wasm)";

    fn open(&mut self, input: DecoderInput) -> SdkResult<Self::Session> {
        NcmWasmDecoderSession::open(input)
    }
}

fn create_plugin() -> SdkResult<NcmWasmDecoderPlugin> {
    Ok(NcmWasmDecoderPlugin)
}

stellatune_wasm_plugin_sdk::export_decoder_component! {
    plugin_type: crate::NcmWasmDecoderPlugin,
    create: crate::create_plugin,
}
