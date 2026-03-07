mod encoder;

use encoder::{FfmpegEncoderSession, probe_sidecar_binaries};
use stellatune_plugin_ffmpeg_common::{
    CONFIG_SCHEMA_JSON, DEFAULT_CONFIG_JSON, ENCODER_DISPLAY_NAME, ENCODER_TYPE_ID,
    FfmpegPluginConfig,
};
use stellatune_plugin_sdk::prelude::*;

pub struct FfmpegEncoderPlugin;

impl PluginLifecycle for FfmpegEncoderPlugin {
    fn on_enable(&mut self) -> SdkResult<()> {
        let _ = probe_sidecar_binaries(&FfmpegPluginConfig::default())?;
        Ok(())
    }
}

impl EncoderPlugin for FfmpegEncoderPlugin {
    type Session = FfmpegEncoderSession;

    const TYPE_ID: &'static str = ENCODER_TYPE_ID;
    const DISPLAY_NAME: &'static str = ENCODER_DISPLAY_NAME;
    const CONFIG_SCHEMA_JSON: &'static str = CONFIG_SCHEMA_JSON;
    const DEFAULT_CONFIG_JSON: &'static str = DEFAULT_CONFIG_JSON;

    fn create_session(
        &mut self,
        input: AudioSpec,
        target: EncodeTarget,
        metadata: Option<MediaMetadata>,
    ) -> SdkResult<Self::Session> {
        FfmpegEncoderSession::new(input, target, metadata)
    }
}

fn create_plugin() -> SdkResult<FfmpegEncoderPlugin> {
    Ok(FfmpegEncoderPlugin)
}

stellatune_plugin_sdk::export_encoder_component! {
    plugin_type: crate::FfmpegEncoderPlugin,
    create: crate::create_plugin,
}
