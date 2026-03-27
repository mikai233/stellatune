use std::collections::BTreeMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::{Condvar, Mutex};
use tracing::{debug, info};

use crate::error::Result;
use crate::host::sidecar::types::{
    SidecarChannelHandle, SidecarHost, SidecarLaunchScope, SidecarLaunchSpec, SidecarProcessHandle,
    SidecarTransportOption,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct SidecarProcessKey {
    pub(super) plugin_id: String,
    pub(super) signature_id: u64,
    signature: String,
}

struct SharedProcessEntry {
    process: Arc<Mutex<Box<dyn SidecarProcessHandle>>>,
    lease_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct SidecarLockKey {
    pub(super) plugin_id: String,
    pub(super) lock_name: String,
}

#[derive(Default)]
struct NamedLockState {
    held: bool,
}

#[derive(Default)]
struct NamedLock {
    state: Mutex<NamedLockState>,
    cv: Condvar,
}

struct PackageSidecarRegistryInner {
    host: Arc<dyn SidecarHost>,
    plugin_refs: Mutex<BTreeMap<String, usize>>,
    processes: Mutex<BTreeMap<SidecarProcessKey, SharedProcessEntry>>,
    locks: Mutex<BTreeMap<SidecarLockKey, Arc<NamedLock>>>,
}

#[derive(Clone)]
pub(crate) struct PackageSidecarRegistry {
    inner: Arc<PackageSidecarRegistryInner>,
}

impl PackageSidecarRegistry {
    pub(crate) fn new(host: Arc<dyn SidecarHost>) -> Self {
        Self {
            inner: Arc::new(PackageSidecarRegistryInner {
                host,
                plugin_refs: Mutex::new(BTreeMap::new()),
                processes: Mutex::new(BTreeMap::new()),
                locks: Mutex::new(BTreeMap::new()),
            }),
        }
    }

    pub(crate) fn plugin_activated(&self, plugin_id: &str) {
        let plugin_id = plugin_id.trim();
        if plugin_id.is_empty() {
            return;
        }
        let leases = {
            let mut refs = self.inner.plugin_refs.lock();
            let entry = refs.entry(plugin_id.to_string()).or_insert(0);
            *entry = entry.saturating_add(1);
            *entry
        };
        debug!(plugin_id, refs = leases, "package sidecar plugin activated");
    }

    pub(crate) fn plugin_deactivated(&self, plugin_id: &str, grace_ms: u32) {
        let plugin_id = plugin_id.trim();
        if plugin_id.is_empty() {
            return;
        }

        let should_force_release = {
            let mut refs = self.inner.plugin_refs.lock();
            let Some(entry) = refs.get_mut(plugin_id) else {
                return;
            };
            if *entry > 1 {
                *entry -= 1;
                debug!(
                    plugin_id,
                    refs = *entry,
                    "package sidecar plugin deactivated (references remain)"
                );
                false
            } else {
                refs.remove(plugin_id);
                true
            }
        };
        if !should_force_release {
            return;
        }

        let processes = {
            let mut processes = self.inner.processes.lock();
            let keys = processes
                .keys()
                .filter(|key| key.plugin_id == plugin_id)
                .cloned()
                .collect::<Vec<_>>();
            let mut removed =
                Vec::<(SidecarProcessKey, Arc<Mutex<Box<dyn SidecarProcessHandle>>>)>::new();
            for key in keys {
                if let Some(entry) = processes.remove(&key) {
                    removed.push((key, entry.process));
                }
            }
            removed
        };
        for (key, process) in processes {
            info!(
                plugin_id = %key.plugin_id,
                signature_id = key.signature_id,
                grace_ms,
                "terminating package sidecar process due to plugin deactivation"
            );
            let mut process = process.lock();
            let _ = process.terminate(grace_ms);
        }
    }

    fn is_plugin_active(&self, plugin_id: &str) -> bool {
        self.inner
            .plugin_refs
            .lock()
            .get(plugin_id)
            .copied()
            .unwrap_or(0)
            > 0
    }

    pub(super) fn acquire_process(
        &self,
        plugin_id: &str,
        spec: &SidecarLaunchSpec,
    ) -> Result<SidecarProcessKey> {
        let signature = launch_signature(spec);
        let key = SidecarProcessKey {
            plugin_id: plugin_id.to_string(),
            signature_id: signature_id(signature.as_str()),
            signature,
        };
        {
            let mut processes = self.inner.processes.lock();
            if let Some(entry) = processes.get_mut(&key) {
                entry.lease_count = entry.lease_count.saturating_add(1);
                debug!(
                    plugin_id = %key.plugin_id,
                    signature_id = key.signature_id,
                    leases = entry.lease_count,
                    "reuse shared sidecar process"
                );
                return Ok(key);
            }
        }

        debug!(
            plugin_id = %key.plugin_id,
            signature_id = key.signature_id,
            executable = %spec.executable,
            "launching shared sidecar process"
        );
        let launched = Arc::new(Mutex::new(self.inner.host.launch(spec)?));

        let mut processes = self.inner.processes.lock();
        if let Some(entry) = processes.get_mut(&key) {
            entry.lease_count = entry.lease_count.saturating_add(1);
            debug!(
                plugin_id = %key.plugin_id,
                signature_id = key.signature_id,
                leases = entry.lease_count,
                "shared sidecar launch raced; reusing existing process and terminating duplicate"
            );
            drop(processes);
            let mut process = launched.lock();
            let _ = process.terminate(0);
            return Ok(key);
        }

        processes.insert(
            key.clone(),
            SharedProcessEntry {
                process: launched,
                lease_count: 1,
            },
        );
        info!(
            plugin_id = %key.plugin_id,
            signature_id = key.signature_id,
            executable = %spec.executable,
            "shared sidecar process launched"
        );
        Ok(key)
    }

    pub(super) fn launch_instance_process(
        &self,
        spec: &SidecarLaunchSpec,
    ) -> Result<Arc<Mutex<Box<dyn SidecarProcessHandle>>>> {
        Ok(Arc::new(Mutex::new(self.inner.host.launch(spec)?)))
    }

    pub(super) fn open_control(
        &self,
        key: &SidecarProcessKey,
    ) -> Result<Box<dyn SidecarChannelHandle>> {
        let process = self.get_process(key)?;
        let mut process = process.lock();
        process.open_control()
    }

    pub(super) fn open_data(
        &self,
        key: &SidecarProcessKey,
        role: &str,
        preferred: &[SidecarTransportOption],
    ) -> Result<Box<dyn SidecarChannelHandle>> {
        let process = self.get_process(key)?;
        let mut process = process.lock();
        process.open_data(role, preferred)
    }

    pub(super) fn wait_exit(
        &self,
        key: &SidecarProcessKey,
        timeout_ms: Option<u32>,
    ) -> Result<Option<i32>> {
        let process = self.get_process(key)?;
        let mut process = process.lock();
        process.wait_exit(timeout_ms)
    }

    pub(super) fn release_process(&self, key: &SidecarProcessKey, grace_ms: u32) -> Result<()> {
        let keep_alive = self.is_plugin_active(key.plugin_id.as_str());
        let process = {
            let mut processes = self.inner.processes.lock();
            let Some(entry) = processes.get_mut(key) else {
                debug!(
                    plugin_id = %key.plugin_id,
                    signature_id = key.signature_id,
                    "skip sidecar release because process key is missing"
                );
                return Ok(());
            };
            if entry.lease_count > 1 {
                entry.lease_count -= 1;
                debug!(
                    plugin_id = %key.plugin_id,
                    signature_id = key.signature_id,
                    leases = entry.lease_count,
                    "released shared sidecar lease"
                );
                return Ok(());
            }
            if keep_alive {
                entry.lease_count = 0;
                debug!(
                    plugin_id = %key.plugin_id,
                    signature_id = key.signature_id,
                    "released shared sidecar lease (kept alive while plugin is active)"
                );
                return Ok(());
            }
            processes.remove(key).map(|entry| entry.process)
        };

        if let Some(process) = process {
            info!(
                plugin_id = %key.plugin_id,
                signature_id = key.signature_id,
                grace_ms,
                "terminating shared sidecar process (last lease released)"
            );
            let mut process = process.lock();
            process.terminate(grace_ms)?;
        }
        Ok(())
    }

    fn get_process(
        &self,
        key: &SidecarProcessKey,
    ) -> Result<Arc<Mutex<Box<dyn SidecarProcessHandle>>>> {
        let processes = self.inner.processes.lock();
        let Some(entry) = processes.get(key) else {
            return Err(crate::op_error!(
                "shared sidecar process not found for plugin `{}`",
                key.plugin_id
            ));
        };
        Ok(entry.process.clone())
    }

    pub(super) fn acquire_lock(
        &self,
        plugin_id: &str,
        lock_name: &str,
        timeout_ms: Option<u32>,
    ) -> Result<SidecarLockKey> {
        let lock_name = lock_name.trim();
        if lock_name.is_empty() {
            return Err(crate::op_error!("sidecar lock name is empty"));
        }
        let key = SidecarLockKey {
            plugin_id: plugin_id.to_string(),
            lock_name: lock_name.to_string(),
        };
        let lock = {
            let mut locks = self.inner.locks.lock();
            locks
                .entry(key.clone())
                .or_insert_with(|| Arc::new(NamedLock::default()))
                .clone()
        };

        let mut state = lock.state.lock();
        if !state.held {
            state.held = true;
            return Ok(key);
        }

        if let Some(timeout_ms) = timeout_ms {
            let timeout = Duration::from_millis(timeout_ms as u64);
            let deadline = Instant::now() + timeout;
            loop {
                let now = Instant::now();
                if now >= deadline {
                    return Err(crate::op_error!(
                        "sidecar lock `{}` timed out after {}ms",
                        lock_name,
                        timeout_ms
                    ));
                }
                let remaining = deadline.saturating_duration_since(now);
                let wait = lock.cv.wait_for(&mut state, remaining);
                if wait.timed_out() && state.held {
                    return Err(crate::op_error!(
                        "sidecar lock `{}` timed out after {}ms",
                        lock_name,
                        timeout_ms
                    ));
                }
                if !state.held {
                    state.held = true;
                    return Ok(key);
                }
            }
        }

        while state.held {
            lock.cv.wait(&mut state);
        }
        state.held = true;
        Ok(key)
    }

    pub(super) fn release_lock(&self, key: &SidecarLockKey) {
        let lock = {
            let locks = self.inner.locks.lock();
            locks.get(key).cloned()
        };
        let Some(lock) = lock else {
            debug!(
                plugin_id = %key.plugin_id,
                lock_name = %key.lock_name,
                "skip sidecar unlock because lock key is missing"
            );
            return;
        };

        let mut state = lock.state.lock();
        if state.held {
            state.held = false;
            lock.cv.notify_one();
        }
    }
}

fn launch_signature(spec: &SidecarLaunchSpec) -> String {
    let mut env = spec.env.clone();
    env.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    let scope = match spec.scope {
        SidecarLaunchScope::Instance => "instance",
        SidecarLaunchScope::Package => "package",
    };
    format!(
        "scope={};exe={};args={:?};control={:?};data={:?};env={:?}",
        scope, spec.executable, spec.args, spec.preferred_control, spec.preferred_data, env
    )
}

fn signature_id(signature: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    signature.hash(&mut hasher);
    hasher.finish()
}
