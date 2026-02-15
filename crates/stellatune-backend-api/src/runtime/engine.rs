use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

use stellatune_audio::{EngineHandle, start_engine};

use super::init_tracing;

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

pub async fn runtime_prepare_hot_restart() {
    let engine = shared_runtime_engine();
    if let Err(err) = engine.stop().await {
        tracing::warn!("runtime_prepare_hot_restart stop failed: {err}");
    }
    if let Err(err) = engine.clear_output_sink_route().await {
        tracing::warn!("runtime_prepare_hot_restart clear route failed: {err}");
    }
}

pub async fn runtime_shutdown() {
    let engine = shared_runtime_engine();
    if let Err(err) = engine.stop().await {
        tracing::warn!("runtime_shutdown stop failed: {err}");
    }
    if let Err(err) = engine.clear_output_sink_route().await {
        tracing::warn!("runtime_shutdown clear route failed: {err}");
    }
    if let Err(err) = engine.shutdown().await {
        tracing::warn!("runtime_shutdown command failed: {err}");
    }
}
