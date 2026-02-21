use std::collections::BTreeMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::{Condvar, Mutex};
use tracing::{debug, info};

use crate::error::Result;
use crate::host::sidecar::{
    SidecarChannelHandle, SidecarHost, SidecarLaunchScope, SidecarLaunchSpec, SidecarProcessHandle,
    SidecarTransportKind, SidecarTransportOption,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct SidecarProcessKey {
    plugin_id: String,
    signature_id: u64,
    signature: String,
}

struct SharedProcessEntry {
    process: Arc<Mutex<Box<dyn SidecarProcessHandle>>>,
    lease_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct SidecarLockKey {
    plugin_id: String,
    lock_name: String,
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

    fn acquire_process(
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

    fn open_control(&self, key: &SidecarProcessKey) -> Result<Box<dyn SidecarChannelHandle>> {
        let process = self.get_process(key)?;
        let mut process = process.lock();
        process.open_control()
    }

    fn open_data(
        &self,
        key: &SidecarProcessKey,
        role: &str,
        preferred: &[SidecarTransportOption],
    ) -> Result<Box<dyn SidecarChannelHandle>> {
        let process = self.get_process(key)?;
        let mut process = process.lock();
        process.open_data(role, preferred)
    }

    fn wait_exit(&self, key: &SidecarProcessKey, timeout_ms: Option<u32>) -> Result<Option<i32>> {
        let process = self.get_process(key)?;
        let mut process = process.lock();
        process.wait_exit(timeout_ms)
    }

    fn release_process(&self, key: &SidecarProcessKey, grace_ms: u32) -> Result<()> {
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

    fn acquire_lock(
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

    fn release_lock(&self, key: &SidecarLockKey) {
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

pub(crate) struct SidecarState {
    registry: PackageSidecarRegistry,
    plugin_id: String,
    next_process_rep: u32,
    next_channel_rep: u32,
    next_lock_rep: u32,
    processes: BTreeMap<u32, SidecarProcessRef>,
    channels: BTreeMap<u32, Box<dyn SidecarChannelHandle>>,
    locks: BTreeMap<u32, SidecarLockKey>,
}

enum SidecarProcessRef {
    Shared(SidecarProcessKey),
    Instance(Arc<Mutex<Box<dyn SidecarProcessHandle>>>),
}

impl SidecarState {
    pub(crate) fn new(plugin_id: String, registry: PackageSidecarRegistry) -> Self {
        Self {
            registry,
            plugin_id,
            next_process_rep: 1,
            next_channel_rep: 1,
            next_lock_rep: 1,
            processes: BTreeMap::new(),
            channels: BTreeMap::new(),
            locks: BTreeMap::new(),
        }
    }

    pub(crate) fn launch(&mut self, spec: &SidecarLaunchSpec) -> Result<u32> {
        let process_ref = match spec.scope {
            SidecarLaunchScope::Package => SidecarProcessRef::Shared(
                self.registry
                    .acquire_process(self.plugin_id.as_str(), spec)?,
            ),
            SidecarLaunchScope::Instance => SidecarProcessRef::Instance(Arc::new(Mutex::new(
                self.registry.inner.host.launch(spec)?,
            ))),
        };
        let process_rep = self.alloc_process_rep();
        self.processes.insert(process_rep, process_ref);
        Ok(process_rep)
    }

    pub(crate) fn open_control(&mut self, process_rep: u32) -> Result<u32> {
        let process = self
            .processes
            .get(&process_rep)
            .ok_or_else(|| crate::op_error!("sidecar process handle `{process_rep}` not found"))?;
        let channel = match process {
            SidecarProcessRef::Shared(key) => self.registry.open_control(key)?,
            SidecarProcessRef::Instance(process) => {
                let mut process = process.lock();
                process.open_control()?
            },
        };
        let channel_rep = self.alloc_channel_rep();
        self.channels.insert(channel_rep, channel);
        Ok(channel_rep)
    }

    pub(crate) fn open_data(
        &mut self,
        process_rep: u32,
        role: &str,
        preferred: &[SidecarTransportOption],
    ) -> Result<u32> {
        let process = self
            .processes
            .get(&process_rep)
            .ok_or_else(|| crate::op_error!("sidecar process handle `{process_rep}` not found"))?;
        let channel = match process {
            SidecarProcessRef::Shared(key) => self.registry.open_data(key, role, preferred)?,
            SidecarProcessRef::Instance(process) => {
                let mut process = process.lock();
                process.open_data(role, preferred)?
            },
        };
        let channel_rep = self.alloc_channel_rep();
        self.channels.insert(channel_rep, channel);
        Ok(channel_rep)
    }

    pub(crate) fn wait_exit(
        &mut self,
        process_rep: u32,
        timeout_ms: Option<u32>,
    ) -> Result<Option<i32>> {
        let process = self
            .processes
            .get(&process_rep)
            .ok_or_else(|| crate::op_error!("sidecar process handle `{process_rep}` not found"))?;
        match process {
            SidecarProcessRef::Shared(key) => self.registry.wait_exit(key, timeout_ms),
            SidecarProcessRef::Instance(process) => {
                let mut process = process.lock();
                process.wait_exit(timeout_ms)
            },
        }
    }

    pub(crate) fn terminate(&mut self, process_rep: u32, grace_ms: u32) -> Result<()> {
        let process = self
            .processes
            .remove(&process_rep)
            .ok_or_else(|| crate::op_error!("sidecar process handle `{process_rep}` not found"))?;
        match process {
            SidecarProcessRef::Shared(key) => self.registry.release_process(&key, grace_ms),
            SidecarProcessRef::Instance(process) => {
                let mut process = process.lock();
                process.terminate(grace_ms)
            },
        }
    }

    pub(crate) fn channel_transport(&mut self, channel_rep: u32) -> Result<SidecarTransportKind> {
        let channel = self
            .channels
            .get_mut(&channel_rep)
            .ok_or_else(|| crate::op_error!("sidecar channel handle `{channel_rep}` not found"))?;
        Ok(channel.transport())
    }

    pub(crate) fn channel_write(&mut self, channel_rep: u32, data: &[u8]) -> Result<u32> {
        let channel = self
            .channels
            .get_mut(&channel_rep)
            .ok_or_else(|| crate::op_error!("sidecar channel handle `{channel_rep}` not found"))?;
        channel.write(data)
    }

    pub(crate) fn channel_read(
        &mut self,
        channel_rep: u32,
        max_bytes: u32,
        timeout_ms: Option<u32>,
    ) -> Result<Vec<u8>> {
        let channel = self
            .channels
            .get_mut(&channel_rep)
            .ok_or_else(|| crate::op_error!("sidecar channel handle `{channel_rep}` not found"))?;
        channel.read(max_bytes, timeout_ms)
    }

    pub(crate) fn channel_close(&mut self, channel_rep: u32) -> Result<()> {
        let channel = self
            .channels
            .get_mut(&channel_rep)
            .ok_or_else(|| crate::op_error!("sidecar channel handle `{channel_rep}` not found"))?;
        channel.close();
        Ok(())
    }

    pub(crate) fn drop_process(&mut self, process_rep: u32) {
        if let Some(process) = self.processes.remove(&process_rep) {
            match process {
                SidecarProcessRef::Shared(key) => {
                    let _ = self.registry.release_process(&key, 0);
                },
                SidecarProcessRef::Instance(process) => {
                    let mut process = process.lock();
                    let _ = process.terminate(0);
                },
            }
        }
    }

    pub(crate) fn drop_channel(&mut self, channel_rep: u32) {
        if let Some(mut channel) = self.channels.remove(&channel_rep) {
            channel.close();
        }
    }

    fn alloc_process_rep(&mut self) -> u32 {
        let rep = self.next_process_rep;
        self.next_process_rep = self.next_process_rep.saturating_add(1);
        if self.next_process_rep == 0 {
            self.next_process_rep = 1;
        }
        rep
    }

    fn alloc_channel_rep(&mut self) -> u32 {
        let rep = self.next_channel_rep;
        self.next_channel_rep = self.next_channel_rep.saturating_add(1);
        if self.next_channel_rep == 0 {
            self.next_channel_rep = 1;
        }
        rep
    }

    pub(crate) fn lock(&mut self, lock_name: &str, timeout_ms: Option<u32>) -> Result<u32> {
        let key = self
            .registry
            .acquire_lock(self.plugin_id.as_str(), lock_name, timeout_ms)?;
        let rep = self.alloc_lock_rep();
        self.locks.insert(rep, key);
        Ok(rep)
    }

    pub(crate) fn unlock(&mut self, lock_rep: u32) -> Result<()> {
        let key = self
            .locks
            .remove(&lock_rep)
            .ok_or_else(|| crate::op_error!("sidecar lock guard `{lock_rep}` not found"))?;
        self.registry.release_lock(&key);
        Ok(())
    }

    pub(crate) fn drop_lock(&mut self, lock_rep: u32) {
        if let Some(key) = self.locks.remove(&lock_rep) {
            self.registry.release_lock(&key);
        }
    }

    fn alloc_lock_rep(&mut self) -> u32 {
        let rep = self.next_lock_rep;
        self.next_lock_rep = self.next_lock_rep.saturating_add(1);
        if self.next_lock_rep == 0 {
            self.next_lock_rep = 1;
        }
        rep
    }
}

impl Drop for SidecarState {
    fn drop(&mut self) {
        for (_, mut channel) in std::mem::take(&mut self.channels) {
            channel.close();
        }
        for (_, process) in std::mem::take(&mut self.processes) {
            match process {
                SidecarProcessRef::Shared(key) => {
                    let _ = self.registry.release_process(&key, 0);
                },
                SidecarProcessRef::Instance(process) => {
                    let mut process = process.lock();
                    let _ = process.terminate(0);
                },
            }
        }
        for (_, key) in std::mem::take(&mut self.locks) {
            self.registry.release_lock(&key);
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use crate::error::Result;
    use crate::executor::sidecar_state::{PackageSidecarRegistry, SidecarState};
    use crate::host::sidecar::{
        SidecarChannelHandle, SidecarHost, SidecarLaunchScope, SidecarLaunchSpec,
        SidecarProcessHandle, SidecarTransportKind, SidecarTransportOption,
    };

    struct FakeChannel {
        kind: SidecarTransportKind,
        closed: bool,
    }

    impl SidecarChannelHandle for FakeChannel {
        fn transport(&self) -> SidecarTransportKind {
            self.kind
        }

        fn write(&mut self, data: &[u8]) -> Result<u32> {
            Ok(data.len() as u32)
        }

        fn read(&mut self, max_bytes: u32, _timeout_ms: Option<u32>) -> Result<Vec<u8>> {
            let size = max_bytes.min(4) as usize;
            Ok(vec![7; size])
        }

        fn close(&mut self) {
            self.closed = true;
        }
    }

    struct FakeProcess;

    impl SidecarProcessHandle for FakeProcess {
        fn open_control(&mut self) -> Result<Box<dyn SidecarChannelHandle>> {
            Ok(Box::new(FakeChannel {
                kind: SidecarTransportKind::Stdio,
                closed: false,
            }))
        }

        fn open_data(
            &mut self,
            _role: &str,
            preferred: &[SidecarTransportOption],
        ) -> Result<Box<dyn SidecarChannelHandle>> {
            let kind = preferred
                .first()
                .map(|option| option.kind)
                .unwrap_or(SidecarTransportKind::LoopbackTcp);
            Ok(Box::new(FakeChannel {
                kind,
                closed: false,
            }))
        }

        fn wait_exit(&mut self, _timeout_ms: Option<u32>) -> Result<Option<i32>> {
            Ok(Some(0))
        }

        fn terminate(&mut self, _grace_ms: u32) -> Result<()> {
            Ok(())
        }
    }

    struct FakeHost;

    impl SidecarHost for FakeHost {
        fn launch(&self, _spec: &SidecarLaunchSpec) -> Result<Box<dyn SidecarProcessHandle>> {
            Ok(Box::new(FakeProcess))
        }
    }

    fn create_state() -> SidecarState {
        let registry = PackageSidecarRegistry::new(Arc::new(FakeHost));
        SidecarState::new("dev.stellatune.test".to_string(), registry)
    }

    #[test]
    fn launch_and_io_path_works() {
        let mut state = create_state();
        let process_rep = state
            .launch(&SidecarLaunchSpec {
                scope: SidecarLaunchScope::Package,
                executable: "demo.exe".to_string(),
                args: Vec::new(),
                preferred_control: Vec::new(),
                preferred_data: Vec::new(),
                env: Vec::new(),
            })
            .expect("launch");
        let channel_rep = state.open_control(process_rep).expect("open control");
        assert_eq!(
            state.channel_transport(channel_rep).expect("transport"),
            SidecarTransportKind::Stdio
        );
        assert_eq!(
            state.channel_write(channel_rep, &[1, 2, 3]).expect("write"),
            3
        );
        assert_eq!(
            state.channel_read(channel_rep, 8, None).expect("read"),
            vec![7, 7, 7, 7]
        );
        state.channel_close(channel_rep).expect("close");
        assert_eq!(
            state.wait_exit(process_rep, Some(100)).expect("wait"),
            Some(0)
        );
        state.terminate(process_rep, 50).expect("terminate");
    }

    #[test]
    fn data_channel_transport_follows_preferred_kind() {
        let kinds = [
            SidecarTransportKind::Stdio,
            SidecarTransportKind::NamedPipe,
            SidecarTransportKind::UnixSocket,
            SidecarTransportKind::LoopbackTcp,
            SidecarTransportKind::SharedMemoryRing,
        ];
        for kind in kinds {
            let mut state = create_state();
            let process_rep = state
                .launch(&SidecarLaunchSpec {
                    scope: SidecarLaunchScope::Package,
                    executable: "demo.exe".to_string(),
                    args: Vec::new(),
                    preferred_control: Vec::new(),
                    preferred_data: Vec::new(),
                    env: Vec::new(),
                })
                .expect("launch");
            let channel_rep = state
                .open_data(
                    process_rep,
                    "sink",
                    &[SidecarTransportOption {
                        kind,
                        priority: 1,
                        max_frame_bytes: None,
                    }],
                )
                .expect("open data");
            assert_eq!(
                state.channel_transport(channel_rep).expect("transport"),
                kind
            );
        }
    }

    #[test]
    fn missing_handles_return_error() {
        let mut state = create_state();
        let error = state
            .open_control(999)
            .expect_err("missing process should fail");
        assert!(error.to_string().contains("not found"));
        let error = state
            .channel_read(999, 16, None)
            .expect_err("missing channel should fail");
        assert!(error.to_string().contains("not found"));
    }

    struct CountingHost {
        launches: Arc<AtomicUsize>,
    }

    impl SidecarHost for CountingHost {
        fn launch(&self, _spec: &SidecarLaunchSpec) -> Result<Box<dyn SidecarProcessHandle>> {
            self.launches.fetch_add(1, Ordering::SeqCst);
            Ok(Box::new(FakeProcess))
        }
    }

    #[test]
    fn share_sidecar_process_within_same_plugin() {
        let launches = Arc::new(AtomicUsize::new(0));
        let registry = PackageSidecarRegistry::new(Arc::new(CountingHost {
            launches: launches.clone(),
        }));
        let mut first = SidecarState::new("dev.stellatune.shared".to_string(), registry.clone());
        let mut second = SidecarState::new("dev.stellatune.shared".to_string(), registry);

        let spec = SidecarLaunchSpec {
            scope: SidecarLaunchScope::Package,
            executable: "demo.exe".to_string(),
            args: vec!["--api".to_string()],
            preferred_control: Vec::new(),
            preferred_data: Vec::new(),
            env: vec![("TOKEN".to_string(), "abc".to_string())],
        };
        let first_rep = first.launch(&spec).expect("first launch");
        let second_rep = second.launch(&spec).expect("second launch");

        assert_eq!(launches.load(Ordering::SeqCst), 1);

        first.drop_process(first_rep);
        assert_eq!(launches.load(Ordering::SeqCst), 1);
        second.drop_process(second_rep);
    }

    #[test]
    fn instance_scope_does_not_share_process() {
        let launches = Arc::new(AtomicUsize::new(0));
        let registry = PackageSidecarRegistry::new(Arc::new(CountingHost {
            launches: launches.clone(),
        }));
        let mut first = SidecarState::new("dev.stellatune.instance".to_string(), registry.clone());
        let mut second = SidecarState::new("dev.stellatune.instance".to_string(), registry);

        let spec = SidecarLaunchSpec {
            scope: SidecarLaunchScope::Instance,
            executable: "demo.exe".to_string(),
            args: vec!["--api".to_string()],
            preferred_control: Vec::new(),
            preferred_data: Vec::new(),
            env: Vec::new(),
        };
        let first_rep = first.launch(&spec).expect("first launch");
        let second_rep = second.launch(&spec).expect("second launch");

        assert_eq!(launches.load(Ordering::SeqCst), 2);

        first.drop_process(first_rep);
        second.drop_process(second_rep);
    }

    #[test]
    fn lock_is_serialized_per_plugin_and_name() {
        let registry = PackageSidecarRegistry::new(Arc::new(FakeHost));
        let mut first =
            SidecarState::new("dev.stellatune.shared-lock".to_string(), registry.clone());
        let mut second = SidecarState::new("dev.stellatune.shared-lock".to_string(), registry);

        let first_lock = first.lock("asio-control", Some(100)).expect("first lock");
        let error = second
            .lock("asio-control", Some(20))
            .expect_err("second lock should timeout while first lock is held");
        assert!(error.to_string().contains("timed out"));

        first.unlock(first_lock).expect("first unlock");

        let second_lock = second
            .lock("asio-control", Some(100))
            .expect("second lock after release");
        second.unlock(second_lock).expect("second unlock");
    }

    #[test]
    fn lock_name_isolated_by_plugin_id() {
        let registry = PackageSidecarRegistry::new(Arc::new(FakeHost));
        let mut first = SidecarState::new("dev.stellatune.alpha".to_string(), registry.clone());
        let mut second = SidecarState::new("dev.stellatune.beta".to_string(), registry);

        let first_lock = first.lock("shared-name", Some(100)).expect("first lock");
        let second_lock = second
            .lock("shared-name", Some(100))
            .expect("second lock should not be blocked by different plugin");

        first.unlock(first_lock).expect("first unlock");
        second.unlock(second_lock).expect("second unlock");
    }
}
