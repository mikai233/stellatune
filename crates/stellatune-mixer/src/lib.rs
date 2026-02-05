//! Channel layout definitions and mixing utilities for multi-channel audio.

mod layout;
mod matrix;
mod mixer;

pub use layout::{Channel, ChannelLayout, LfeMode};
pub use matrix::MixMatrix;
pub use mixer::ChannelMixer;
