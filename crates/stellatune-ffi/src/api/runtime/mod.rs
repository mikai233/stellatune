pub async fn prepare_hot_restart() {
    stellatune_backend_api::runtime::runtime_prepare_hot_restart().await;
}

pub async fn shutdown() {
    stellatune_backend_api::runtime::runtime_shutdown().await;
}
