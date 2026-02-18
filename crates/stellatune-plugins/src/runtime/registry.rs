use crate::runtime::model::{AcquiredModuleLease, ModuleLease};

pub(crate) struct ModuleLeaseSlotEntry {
    pub(crate) lease: ModuleLease,
    pub(crate) external_refcount: usize,
}

impl ModuleLeaseSlotEntry {
    pub(crate) fn new(lease: ModuleLease) -> Self {
        Self {
            lease,
            external_refcount: 0,
        }
    }

    pub(crate) fn lease_id(&self) -> u64 {
        self.lease.lease_id
    }

    pub(crate) fn acquire(&mut self) -> AcquiredModuleLease {
        self.external_refcount = self.external_refcount.saturating_add(1);
        AcquiredModuleLease {
            lease_id: self.lease.lease_id,
            module: self.lease.loaded.module,
        }
    }

    pub(crate) fn release(&mut self) -> bool {
        if self.external_refcount == 0 {
            return false;
        }
        self.external_refcount -= 1;
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ReleasedLeaseState {
    pub(crate) external_refcount: usize,
    pub(crate) was_retired: bool,
}

#[derive(Default)]
pub(crate) struct PluginModuleLeaseSlotState {
    pub(crate) current: Option<ModuleLeaseSlotEntry>,
    pub(crate) retired: Vec<ModuleLeaseSlotEntry>,
}

impl PluginModuleLeaseSlotState {
    pub(crate) fn set_current(&mut self, next: ModuleLease) {
        if let Some(cur) = self.current.take() {
            self.retired.push(cur);
        }
        self.current = Some(ModuleLeaseSlotEntry::new(next));
    }

    pub(crate) fn retire_current(&mut self) -> bool {
        if let Some(cur) = self.current.take() {
            self.retired.push(cur);
            return true;
        }
        false
    }

    pub(crate) fn acquire_current(&mut self) -> Option<AcquiredModuleLease> {
        self.current.as_mut().map(ModuleLeaseSlotEntry::acquire)
    }

    pub(crate) fn release_lease(&mut self, lease_id: u64) -> Option<ReleasedLeaseState> {
        if let Some(current) = self.current.as_mut()
            && current.lease_id() == lease_id
        {
            if !current.release() {
                return None;
            }
            return Some(ReleasedLeaseState {
                external_refcount: current.external_refcount,
                was_retired: false,
            });
        }
        for retired in &mut self.retired {
            if retired.lease_id() != lease_id {
                continue;
            }
            if !retired.release() {
                return None;
            }
            return Some(ReleasedLeaseState {
                external_refcount: retired.external_refcount,
                was_retired: true,
            });
        }
        None
    }
}
