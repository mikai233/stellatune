pub(super) mod handlers;

use std::sync::Arc;

use crate::runtime::types::{PendingControlFinish, PluginRuntimeRouter};

pub(super) struct RuntimeRouterActor {
    pub(super) router: Arc<PluginRuntimeRouter>,
    pub(super) engine: Option<stellatune_audio::EngineHandle>,
    pub(super) library: Option<stellatune_library::LibraryHandle>,
    pub(super) pending_finishes: Vec<PendingControlFinish>,
}
