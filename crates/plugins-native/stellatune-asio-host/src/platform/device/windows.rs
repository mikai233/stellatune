pub(crate) fn asio_host() -> Result<cpal::Host, String> {
    cpal::host_from_id(cpal::HostId::Asio).map_err(|e| e.to_string())
}
