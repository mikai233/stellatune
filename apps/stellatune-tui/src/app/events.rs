use stellatune_audio::playback::event::{PlaybackEvent as AudioEvent, PlaybackState};
use stellatune_library::LibraryEvent;

use super::App;

impl App {
    pub(super) fn handle_engine_event(&mut self, event: AudioEvent) {
        match event {
            AudioEvent::StateChanged(state) => {
                self.state.playback.player_state = state;
            },
            AudioEvent::TrackChanged { .. } => {
                self.state.playback.position_ms = 0;
            },
            AudioEvent::PlaybackEnded { .. } => {
                self.state.playback.player_state = PlaybackState::Idle;
            },
            AudioEvent::Position { position, .. } => {
                self.state.playback.position_ms = position.as_millis() as i64;
            },
            AudioEvent::Failed(failure) => {
                self.toast_error(format!("playback error: {}", failure.message));
            },
            AudioEvent::Buffering { active, .. } if active => {
                self.toast_warn("playback buffering");
            },
            AudioEvent::Buffering { .. } => {},
        }
    }

    pub(super) async fn handle_library_event(&mut self, event: LibraryEvent) {
        match event {
            LibraryEvent::Changed => {
                if let Err(error) = self.refresh_library().await {
                    self.toast_error(format!("library refresh failed: {error}"));
                }
            },
            LibraryEvent::ScanProgress {
                scanned,
                updated,
                skipped,
                errors,
            } => {
                self.state.library.scan_progress = Some(format!(
                    "scanned={scanned}, updated={updated}, skipped={skipped}, errors={errors}"
                ));
            },
            LibraryEvent::ScanFinished {
                duration_ms,
                scanned,
                updated,
                skipped,
                errors,
            } => {
                self.state.library.scan_progress = None;
                self.state.status_line = format!(
                    "scan finished in {duration_ms}ms (scanned={scanned}, updated={updated}, skipped={skipped}, errors={errors})"
                );
                if let Err(error) = self.refresh_library().await {
                    self.state.status_line = format!("library refresh failed: {error}");
                }
            },
            LibraryEvent::Error { message } => {
                self.toast_error(format!("library error: {message}"));
            },
            LibraryEvent::Log { message } => {
                self.state.status_line = message;
            },
        }
    }
}
