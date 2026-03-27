pub(crate) fn asio_host() -> Result<cpal::Host, String> {
    Err("ASIO support not built (enable `stellatune-asio-host` feature `asio`)".to_string())
}
