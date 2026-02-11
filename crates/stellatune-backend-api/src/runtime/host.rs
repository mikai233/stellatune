use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use stellatune_audio::{EngineHandle, start_engine};
use stellatune_core::Command;

use super::{init_tracing, register_plugin_runtime_engine};

pub struct RuntimeHost {
    host_id: u64,
    engine: EngineHandle,
    client_generation: AtomicU64,
    next_client_id: AtomicU64,
    clients: Mutex<HashMap<u64, RuntimeClientEntry>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RuntimeClientId {
    generation: u64,
    id: u64,
}

impl RuntimeClientId {
    pub fn as_u64(self) -> u64 {
        self.id
    }

    pub fn generation(self) -> u64 {
        self.generation
    }
}

#[derive(Debug, Clone)]
struct RuntimeClientEntry {
    attached_at: Instant,
}

impl RuntimeHost {
    fn new() -> Self {
        static NEXT_HOST_ID: AtomicU64 = AtomicU64::new(1);

        init_tracing();
        let host_id = NEXT_HOST_ID.fetch_add(1, Ordering::Relaxed);
        tracing::info!(host_id, "initializing runtime host");

        let engine = start_engine();
        register_plugin_runtime_engine(engine.clone());

        Self {
            host_id,
            engine,
            client_generation: AtomicU64::new(1),
            next_client_id: AtomicU64::new(1),
            clients: Mutex::new(HashMap::new()),
        }
    }

    pub fn host_id(&self) -> u64 {
        self.host_id
    }

    pub fn engine(&self) -> &EngineHandle {
        &self.engine
    }

    pub fn attach_client(&self) -> RuntimeClientId {
        let client_id = RuntimeClientId {
            generation: self.client_generation.load(Ordering::Relaxed),
            id: self.next_client_id.fetch_add(1, Ordering::Relaxed),
        };
        let active_clients = {
            let mut clients = self
                .clients
                .lock()
                .expect("runtime host clients mutex poisoned");
            clients.insert(
                client_id.id,
                RuntimeClientEntry {
                    attached_at: Instant::now(),
                },
            );
            clients.len()
        };
        tracing::info!(
            runtime_host_id = self.host_id,
            client_id = client_id.id,
            generation = client_id.generation,
            active_clients,
            "runtime client attached"
        );
        client_id
    }

    pub fn detach_client(&self, client_id: RuntimeClientId) {
        let (removed, active_clients, attached_for_ms) = {
            let mut clients = self
                .clients
                .lock()
                .expect("runtime host clients mutex poisoned");
            let removed = clients.remove(&client_id.id);
            let attached_for_ms = removed
                .as_ref()
                .map(|v| v.attached_at.elapsed().as_millis() as u64)
                .unwrap_or(0);
            (removed.is_some(), clients.len(), attached_for_ms)
        };
        if removed {
            tracing::info!(
                runtime_host_id = self.host_id,
                client_id = client_id.id,
                generation = client_id.generation,
                active_clients,
                attached_for_ms,
                "runtime client detached"
            );
        } else if client_id.generation < self.client_generation.load(Ordering::Relaxed) {
            tracing::info!(
                runtime_host_id = self.host_id,
                client_id = client_id.id,
                generation = client_id.generation,
                active_clients,
                "runtime client detach ignored; already evicted by generation rollover"
            );
        } else {
            tracing::warn!(
                runtime_host_id = self.host_id,
                client_id = client_id.id,
                generation = client_id.generation,
                active_clients,
                "runtime client detach ignored; client not found"
            );
        }
    }

    pub fn active_client_count(&self) -> usize {
        self.clients.lock().map(|m| m.len()).unwrap_or_default()
    }

    pub fn prepare_hot_restart(&self) {
        let next_generation = self.client_generation.fetch_add(1, Ordering::Relaxed) + 1;
        let evicted_clients = {
            let mut clients = self
                .clients
                .lock()
                .expect("runtime host clients mutex poisoned");
            let count = clients.len();
            clients.clear();
            count
        };
        self.engine.send_command(Command::Stop);
        self.engine.send_command(Command::ClearOutputSinkRoute);
        tracing::info!(
            runtime_host_id = self.host_id,
            evicted_clients,
            generation = next_generation,
            "runtime host prepared for hot restart"
        );
    }

    pub fn shutdown(&self) {
        self.prepare_hot_restart();
        self.engine.send_command(Command::Shutdown);
        tracing::info!(
            runtime_host_id = self.host_id,
            "runtime host shutdown requested"
        );
    }
}

pub fn shared_runtime_host() -> Arc<RuntimeHost> {
    static HOST: OnceLock<Arc<RuntimeHost>> = OnceLock::new();
    Arc::clone(HOST.get_or_init(|| Arc::new(RuntimeHost::new())))
}
