pub(crate) mod handlers;

use std::sync::Arc;

use super::EventHub;
use crate::worker::LibraryWorker;

pub(crate) struct LibraryServiceActor {
    pub(crate) worker: LibraryWorker,
    pub(crate) events: Arc<EventHub>,
}

impl LibraryServiceActor {
    pub(crate) fn new(worker: LibraryWorker, events: Arc<EventHub>) -> Self {
        Self { worker, events }
    }
}
