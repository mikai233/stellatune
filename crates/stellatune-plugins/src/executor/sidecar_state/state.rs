use std::collections::BTreeMap;
use std::sync::Arc;

use parking_lot::Mutex;

use crate::error::Result;
use crate::host::sidecar::types::{
    SidecarChannelHandle, SidecarLaunchScope, SidecarLaunchSpec, SidecarProcessHandle,
    SidecarTransportKind, SidecarTransportOption,
};

use super::registry::{PackageSidecarRegistry, SidecarLockKey, SidecarProcessKey};

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
            SidecarLaunchScope::Instance => {
                SidecarProcessRef::Instance(self.registry.launch_instance_process(spec)?)
            },
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
