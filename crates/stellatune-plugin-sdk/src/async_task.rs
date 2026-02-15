use core::future::Future;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, TryRecvError};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use tokio::runtime::Runtime;
use tokio::task::JoinHandle;

use crate::{SdkResult, StAsyncOpState, StOpNotifier};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AsyncTaskTakeError {
    Pending,
    Cancelled,
    Failed(String),
    AlreadyTaken,
}

struct AsyncTaskOpInner<R> {
    state: StAsyncOpState,
    rx: Option<Receiver<SdkResult<R>>>,
    result: Option<SdkResult<R>>,
    join: Option<JoinHandle<()>>,
    notifier: Option<StOpNotifier>,
}

pub struct AsyncTaskOp<R> {
    inner: Mutex<AsyncTaskOpInner<R>>,
}

impl<R: Send + 'static> AsyncTaskOp<R> {
    pub fn spawn<F>(future: F) -> Self
    where
        F: Future<Output = SdkResult<R>> + Send + 'static,
    {
        let (tx, rx) = mpsc::channel();
        let join = runtime().spawn(async move {
            let output = future.await;
            let _ = tx.send(output);
        });
        Self {
            inner: Mutex::new(AsyncTaskOpInner {
                state: StAsyncOpState::Pending,
                rx: Some(rx),
                result: None,
                join: Some(join),
                notifier: None,
            }),
        }
    }

    pub fn poll(&self) -> StAsyncOpState {
        let (state, notifier) = {
            let mut inner = self.lock_inner();
            let notifier = self.try_complete(&mut inner);
            (inner.state, notifier)
        };
        notify_if_needed(notifier);
        state
    }

    pub fn wait(&self, timeout_ms: u32) -> StAsyncOpState {
        let (state, notifier) = {
            let mut inner = self.lock_inner();
            let mut notifier = self.try_complete(&mut inner);
            if inner.state == StAsyncOpState::Pending {
                if timeout_ms == 0 {
                    return inner.state;
                }
                let wait_for = Duration::from_millis(u64::from(timeout_ms));
                let recv_result = inner
                    .rx
                    .as_ref()
                    .map(|rx| rx.recv_timeout(wait_for))
                    .unwrap_or(Err(RecvTimeoutError::Disconnected));
                notifier = match recv_result {
                    Ok(result) => self.complete_with(&mut inner, result),
                    Err(RecvTimeoutError::Timeout) => notifier,
                    Err(RecvTimeoutError::Disconnected) => {
                        self.complete_with_err(&mut inner, "async operation channel disconnected")
                    },
                };
            }
            (inner.state, notifier)
        };
        notify_if_needed(notifier);
        state
    }

    pub fn cancel(&self) -> StAsyncOpState {
        let notifier = {
            let mut inner = self.lock_inner();
            if inner.state != StAsyncOpState::Pending {
                return inner.state;
            }
            if let Some(join) = inner.join.take() {
                join.abort();
            }
            inner.rx = None;
            inner.result = None;
            inner.state = StAsyncOpState::Cancelled;
            inner.notifier
        };
        notify_if_needed(notifier);
        StAsyncOpState::Cancelled
    }

    pub fn set_notifier(&self, notifier: StOpNotifier) {
        let notify_now = {
            let mut inner = self.lock_inner();
            inner.notifier = Some(notifier);
            inner.state != StAsyncOpState::Pending
        };
        if notify_now {
            notify_if_needed(Some(notifier));
        }
    }

    pub fn take_result(&self) -> Result<R, AsyncTaskTakeError> {
        let _ = self.poll();
        let mut inner = self.lock_inner();
        match inner.state {
            StAsyncOpState::Pending => Err(AsyncTaskTakeError::Pending),
            StAsyncOpState::Cancelled => Err(AsyncTaskTakeError::Cancelled),
            StAsyncOpState::Ready => match inner.result.take() {
                Some(Ok(v)) => Ok(v),
                Some(Err(e)) => Err(AsyncTaskTakeError::Failed(e.to_string())),
                None => Err(AsyncTaskTakeError::AlreadyTaken),
            },
            StAsyncOpState::Failed => match inner.result.take() {
                Some(Err(e)) => Err(AsyncTaskTakeError::Failed(e.to_string())),
                Some(Ok(_)) => Err(AsyncTaskTakeError::Failed(
                    "async operation entered failed state".to_string(),
                )),
                None => Err(AsyncTaskTakeError::Failed(
                    "async operation failed".to_string(),
                )),
            },
        }
    }

    fn lock_inner(&self) -> MutexGuard<'_, AsyncTaskOpInner<R>> {
        self.inner
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    fn try_complete(&self, inner: &mut AsyncTaskOpInner<R>) -> Option<StOpNotifier> {
        if inner.state != StAsyncOpState::Pending {
            return None;
        }
        let recv_result = inner
            .rx
            .as_ref()
            .map(|rx| rx.try_recv())
            .unwrap_or(Err(TryRecvError::Disconnected));
        match recv_result {
            Ok(result) => self.complete_with(inner, result),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.complete_with_err(inner, "async operation channel disconnected")
            },
        }
    }

    fn complete_with(
        &self,
        inner: &mut AsyncTaskOpInner<R>,
        result: SdkResult<R>,
    ) -> Option<StOpNotifier> {
        inner.result = Some(result);
        inner.rx = None;
        inner.join = None;
        inner.state = if inner.result.as_ref().is_some_and(Result::is_ok) {
            StAsyncOpState::Ready
        } else {
            StAsyncOpState::Failed
        };
        inner.notifier
    }

    fn complete_with_err(
        &self,
        inner: &mut AsyncTaskOpInner<R>,
        message: &str,
    ) -> Option<StOpNotifier> {
        self.complete_with(inner, Err(crate::SdkError::msg(message)))
    }
}

impl<R> Drop for AsyncTaskOp<R> {
    fn drop(&mut self) {
        let mut inner = self
            .inner
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if let Some(join) = inner.join.take() {
            join.abort();
        }
        inner.rx = None;
    }
}

pub fn runtime() -> &'static Runtime {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("failed to build sdk async runtime")
    })
}

fn notify_if_needed(notifier: Option<StOpNotifier>) {
    let Some(notifier) = notifier else {
        return;
    };
    let Some(cb) = notifier.notify else {
        return;
    };
    cb(notifier.user_data);
}
