use std::sync::OnceLock;

use stellatune_audio::planner::{GainCurve, PlaybackPolicies, TransitionPolicy};
use stellatune_audio_builtin_adapters::device_sink::DeviceSinkControl;
use stellatune_audio_core::error::PlaybackControlError;

use super::shared_playback_controller;

pub fn shared_device_sink_control() -> DeviceSinkControl {
    static CONTROL: OnceLock<DeviceSinkControl> = OnceLock::new();
    CONTROL.get_or_init(DeviceSinkControl::default).clone()
}

pub async fn set_runtime_builtin_transform_options(
    gapless: bool,
    seek_fade: bool,
) -> Result<(), PlaybackControlError> {
    let transition = if gapless {
        TransitionPolicy::Gapless
    } else {
        TransitionPolicy::FadeOutIn {
            fade_out_frames: 5_760,
            fade_in_frames: 5_760,
            curve: GainCurve::EqualPower,
        }
    };
    shared_playback_controller()
        .set_policies(PlaybackPolicies {
            transition,
            seek_fade_frames: if seek_fade { 240 } else { 0 },
            ..PlaybackPolicies::default()
        })
        .await
}
