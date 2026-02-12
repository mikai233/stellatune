use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

use stellatune_audio::{EngineHandle, start_engine};
use stellatune_core::Command;

use super::{init_tracing, register_plugin_runtime_engine};

struct RuntimeEngineMetrics {
    runtime_engine_inits_total: AtomicU64,
}

impl RuntimeEngineMetrics {
    fn new() -> Self {
        Self {
            runtime_engine_inits_total: AtomicU64::new(0),
        }
    }
}

fn runtime_engine_metrics() -> &'static RuntimeEngineMetrics {
    static METRICS: OnceLock<RuntimeEngineMetrics> = OnceLock::new();
    METRICS.get_or_init(RuntimeEngineMetrics::new)
}

fn new_runtime_engine() -> Arc<EngineHandle> {
    init_tracing();
    let inits_total = runtime_engine_metrics()
        .runtime_engine_inits_total
        .fetch_add(1, Ordering::Relaxed)
        + 1;

    let engine = start_engine();
    register_plugin_runtime_engine(engine.clone());

    tracing::info!(
        runtime_engine_inits_total = inits_total,
        "runtime engine initialized"
    );
    Arc::new(engine)
}

pub fn shared_runtime_engine() -> Arc<EngineHandle> {
    static ENGINE: OnceLock<Arc<EngineHandle>> = OnceLock::new();
    if let Some(engine) = ENGINE.get() {
        tracing::debug!("reusing shared runtime engine");
        return Arc::clone(engine);
    }
    Arc::clone(ENGINE.get_or_init(new_runtime_engine))
}

pub fn runtime_prepare_hot_restart() {
    let engine = shared_runtime_engine();
    if let Err(err) = engine.dispatch_command_blocking(Command::Stop) {
        tracing::warn!("runtime_prepare_hot_restart stop failed: {err}");
    }
    if let Err(err) = engine.dispatch_command_blocking(Command::ClearOutputSinkRoute) {
        tracing::warn!("runtime_prepare_hot_restart clear route failed: {err}");
    }
}

pub fn runtime_shutdown() {
    let engine = shared_runtime_engine();
    if let Err(err) = engine.dispatch_command_blocking(Command::Stop) {
        tracing::warn!("runtime_shutdown stop failed: {err}");
    }
    if let Err(err) = engine.dispatch_command_blocking(Command::ClearOutputSinkRoute) {
        tracing::warn!("runtime_shutdown clear route failed: {err}");
    }
    if let Err(err) = engine.dispatch_command_blocking(Command::Shutdown) {
        tracing::warn!("runtime_shutdown command failed: {err}");
    }
}
