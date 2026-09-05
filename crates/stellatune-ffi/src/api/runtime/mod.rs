pub async fn shutdown() {
    let _ = crate::api::player::host_api_stop().await;
    stellatune_backend_api::runtime::runtime_shutdown().await;
}
