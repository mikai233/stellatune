pub(super) mod handlers;

use super::{CachedLyricsInstance, RuntimeInstanceSlotKey};

pub(super) struct LyricsOwnerActor {
    pub(super) slot: RuntimeInstanceSlotKey,
    pub(super) frozen: bool,
    pub(super) entry: Option<CachedLyricsInstance>,
}

impl LyricsOwnerActor {
    pub(super) fn new(plugin_id: String, type_id: String) -> Self {
        Self {
            slot: RuntimeInstanceSlotKey { plugin_id, type_id },
            frozen: false,
            entry: None,
        }
    }
}
