//! Output thread ownership; joining happens outside the playback actor.
use std::sync::Mutex;
use std::thread::JoinHandle;

#[derive(Default)]
pub(super) struct OutputWorkers {
    threads: Mutex<Vec<JoinHandle<()>>>,
    pub(super) device: std::sync::Arc<Mutex<()>>,
}

impl OutputWorkers {
    pub(super) fn register(&self, worker: JoinHandle<()>) {
        let mut workers = self
            .threads
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let mut index = 0;
        while index < workers.len() {
            if workers[index].is_finished() {
                let _ = workers.swap_remove(index).join();
            } else {
                index += 1;
            }
        }
        workers.push(worker);
    }

    pub(super) fn join(&self) {
        let workers = std::mem::take(
            &mut *self
                .threads
                .lock()
                .unwrap_or_else(|error| error.into_inner()),
        );
        for worker in workers {
            let _ = worker.join();
        }
    }
}
