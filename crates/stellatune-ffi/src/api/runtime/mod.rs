pub async fn shutdown() {
    stellatune_backend_api::runtime::runtime_shutdown().await;
}
