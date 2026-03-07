mod decoder;

use decoder::{FfmpegDecoderSession, probe_sidecar_binaries};
use stellatune_plugin_ffmpeg_common::{
    CONFIG_SCHEMA_JSON, DECODER_DISPLAY_NAME, DECODER_TYPE_ID, DEFAULT_CONFIG_JSON,
    FfmpegPluginConfig,
};
use stellatune_plugin_sdk::prelude::*;

pub struct FfmpegDecoderPlugin;

impl PluginLifecycle for FfmpegDecoderPlugin {
    fn on_enable(&mut self) -> SdkResult<()> {
        let _ = probe_sidecar_binaries(&FfmpegPluginConfig::default())?;
        Ok(())
    }
}

impl DecoderPlugin for FfmpegDecoderPlugin {
    type Session = FfmpegDecoderSession;

    const TYPE_ID: &'static str = DECODER_TYPE_ID;
    const DISPLAY_NAME: &'static str = DECODER_DISPLAY_NAME;
    const CONFIG_SCHEMA_JSON: &'static str = CONFIG_SCHEMA_JSON;
    const DEFAULT_CONFIG_JSON: &'static str = DEFAULT_CONFIG_JSON;

    fn open(&mut self, input: DecoderInput) -> SdkResult<Self::Session> {
        FfmpegDecoderSession::open(input)
    }
}

fn create_plugin() -> SdkResult<FfmpegDecoderPlugin> {
    Ok(FfmpegDecoderPlugin)
}

stellatune_plugin_sdk::export_decoder_component! {
    plugin_type: crate::FfmpegDecoderPlugin,
    create: crate::create_plugin,
}
