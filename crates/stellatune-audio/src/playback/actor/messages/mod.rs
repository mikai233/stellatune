//! Each message module owns its payload and PlaybackActor handler.
//! Message paths are explicit; this module does not re-export request types.

use stellatune_audio_core::error::PlaybackControlError;

pub(super) type ControlResult = Result<(), PlaybackControlError>;

pub(in crate::playback) mod advance_to_next;
pub(in crate::playback) mod get_snapshot;
pub(in crate::playback) mod pause;
pub(in crate::playback) mod play;
pub(in crate::playback) mod preparation_completed;
pub(in crate::playback) mod preparation_deadline_elapsed;
pub(in crate::playback) mod pump_audio;
pub(in crate::playback) mod rebuild_output;
pub(in crate::playback) mod recovery_completed;
pub(in crate::playback) mod seek;
pub(in crate::playback) mod set_next;
pub(in crate::playback) mod set_output_gain;
pub(in crate::playback) mod set_policies;
pub(in crate::playback) mod stop_playback;
pub(in crate::playback) mod switch_to;

pub(in crate::playback) mod set_buffering;
