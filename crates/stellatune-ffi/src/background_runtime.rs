use std::future::Future;
use std::sync::OnceLock;

use tokio::runtime::{Builder, Runtime};
use tokio::task::JoinHandle;

fn runtime() -> &'static Runtime {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        Builder::new_multi_thread()
            .enable_all()
            .thread_name("stellatune-ffi-background")
            .build()
            .expect("failed to build FFI background runtime")
    })
}

pub(crate) fn spawn<F>(future: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    runtime().spawn(future)
}

#[cfg(test)]
mod tests {
    #[test]
    fn spawn_works_without_a_caller_tokio_context() {
        let (sender, receiver) = std::sync::mpsc::channel();
        super::spawn(async move {
            sender.send(42).expect("test receiver should remain open");
        });

        assert_eq!(
            receiver
                .recv_timeout(std::time::Duration::from_secs(1))
                .expect("background future should run"),
            42
        );
    }
}
