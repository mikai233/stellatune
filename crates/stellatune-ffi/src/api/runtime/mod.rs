pub fn prepare_hot_restart() {
    stellatune_backend_api::runtime::runtime_prepare_hot_restart();
}

pub fn shutdown() {
    stellatune_backend_api::runtime::runtime_shutdown();
}
