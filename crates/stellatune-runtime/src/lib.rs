use std::future::Future;
use std::sync::OnceLock;

use tokio::runtime::{Builder, Runtime};
use tokio::task::JoinHandle;

pub mod thread_actor;
pub mod tokio_actor;

fn runtime() -> &'static Runtime {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        Builder::new_multi_thread()
            .enable_all()
            .thread_name("stellatune-runtime")
            .build()
            .expect("failed to build shared tokio runtime")
    })
}

pub fn spawn<F>(future: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    runtime().spawn(future)
}

pub fn block_on<F: Future>(future: F) -> F::Output {
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        // Support nested calls from code that is already running inside a Tokio context.
        // This avoids "Cannot start a runtime from within a runtime" panics.
        tokio::task::block_in_place(|| handle.block_on(future))
    } else {
        runtime().block_on(future)
    }
}
