use std::sync::Arc;

use crate::runtime::model::ModuleLease;

#[derive(Default)]
pub(crate) struct PluginModuleLeaseSlotState {
    pub(crate) current: Option<Arc<ModuleLease>>,
    pub(crate) retired: Vec<Arc<ModuleLease>>,
}

impl PluginModuleLeaseSlotState {
    pub(crate) fn set_current(&mut self, next: ModuleLease) {
        if let Some(cur) = self.current.take() {
            self.retired.push(cur);
        }
        self.current = Some(Arc::new(next));
    }

    pub(crate) fn retire_current(&mut self) -> bool {
        if let Some(cur) = self.current.take() {
            self.retired.push(cur);
            return true;
        }
        false
    }
}
