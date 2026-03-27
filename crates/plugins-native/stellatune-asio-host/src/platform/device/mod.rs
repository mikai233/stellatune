#[cfg(not(all(windows, feature = "asio")))]
mod stub;
#[cfg(all(windows, feature = "asio"))]
mod windows;

#[cfg(not(all(windows, feature = "asio")))]
pub(crate) use self::stub::asio_host;
#[cfg(all(windows, feature = "asio"))]
pub(crate) use self::windows::asio_host;
