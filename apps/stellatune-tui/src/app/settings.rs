use anyhow::Result;

use super::{App, Route, format_resample_quality, next_resample_quality, on_off};

impl App {
    pub(super) async fn adjust_settings(&mut self, forward: bool) {
        if self.state.route != Route::Settings {
            return;
        }

        match self.state.settings.selected {
            0 => {
                self.state.settings.resample_quality =
                    next_resample_quality(self.state.settings.resample_quality, forward);
            },
            1 => {
                self.state.settings.match_track_sample_rate =
                    !self.state.settings.match_track_sample_rate;
            },
            2 => {
                self.state.settings.gapless_playback = !self.state.settings.gapless_playback;
            },
            3 => {
                self.state.settings.seek_track_fade = !self.state.settings.seek_track_fade;
            },
            _ => {},
        }

        match self.apply_audio_settings().await {
            Ok(()) => {
                self.state.status_line = format!(
                    "audio settings applied: quality={} match={} gapless={} seek_fade={}",
                    format_resample_quality(self.state.settings.resample_quality),
                    on_off(self.state.settings.match_track_sample_rate),
                    on_off(self.state.settings.gapless_playback),
                    on_off(self.state.settings.seek_track_fade),
                );
            },
            Err(error) => {
                self.state.status_line = format!("apply audio settings failed: {error}");
            },
        }
    }

    pub(super) async fn apply_audio_settings(&self) -> Result<()> {
        self.backend
            .set_audio_output_settings(
                self.state.settings.match_track_sample_rate,
                self.state.settings.resample_quality,
                self.state.settings.gapless_playback,
                self.state.settings.seek_track_fade,
            )
            .await
    }
}
