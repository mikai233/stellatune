#[cfg(not(windows))]
mod stub;
#[cfg(windows)]
mod windows;

#[cfg(not(windows))]
pub(crate) use self::stub::configure_audio_process;
#[cfg(windows)]
pub(crate) use self::windows::configure_audio_process;
