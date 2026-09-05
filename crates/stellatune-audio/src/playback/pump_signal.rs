//! Coalesces readiness notifications into at most one pending pump message.
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::task::{Wake, Waker};
use tokio::sync::Notify;

#[derive(Default)]
pub(super) struct PumpSignal {
    scheduled: AtomicBool,
    notified: Notify,
}
impl PumpSignal {
    pub(super) fn request(&self) {
        if !self.scheduled.swap(true, Ordering::AcqRel) {
            self.notified.notify_one();
        }
    }
    pub(super) async fn notified(&self) {
        self.notified.notified().await;
    }
    pub(super) fn begin_turn(&self) {
        self.scheduled.store(false, Ordering::Release);
    }
    pub(super) fn waker(self: &Arc<Self>) -> Waker {
        Waker::from(self.clone())
    }
}
impl Wake for PumpSignal {
    fn wake(self: Arc<Self>) {
        self.request();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.request();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn wakes_are_coalesced_and_a_wake_during_a_turn_is_retained() {
        let signal = Arc::new(PumpSignal::default());
        for _ in 0..100 {
            signal.waker().wake();
        }
        signal.notified().await;
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(10), signal.notified())
                .await
                .is_err()
        );
        signal.begin_turn();
        signal.waker().wake();
        tokio::time::timeout(std::time::Duration::from_secs(1), signal.notified())
            .await
            .unwrap();
    }
}
