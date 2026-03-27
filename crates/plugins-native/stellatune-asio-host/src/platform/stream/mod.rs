#[cfg(not(windows))]
mod stub;
#[cfg(windows)]
mod windows;

#[cfg(not(windows))]
pub(crate) use self::stub::OutputCallbackPlatformState;
#[cfg(windows)]
pub(crate) use self::windows::OutputCallbackPlatformState;
