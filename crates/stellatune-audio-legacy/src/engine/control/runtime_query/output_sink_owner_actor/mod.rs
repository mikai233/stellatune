pub(super) mod handlers;

use super::{CachedOutputSinkInstance, RuntimeInstanceSlotKey};

pub(crate) struct OutputSinkOwnerActor {
    pub(super) slot: RuntimeInstanceSlotKey,
    pub(super) frozen: bool,
    pub(super) entry: Option<CachedOutputSinkInstance>,
}

impl OutputSinkOwnerActor {
    pub(super) fn new(plugin_id: String, type_id: String) -> Self {
        Self {
            slot: RuntimeInstanceSlotKey { plugin_id, type_id },
            frozen: false,
            entry: None,
        }
    }
}
