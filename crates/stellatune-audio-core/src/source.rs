use std::future::Future;
use std::io::{Cursor, Read, Seek};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::time::Instant;

use crate::{SourceDescriptor, SourceError};

pub trait EncodedSource: Read + Seek + Send {
    fn byte_len(&self) -> Option<u64>;
    fn is_seekable(&self) -> bool;
}

pub type SourceOpenFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Box<dyn EncodedSource>, SourceError>> + Send + 'a>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceOpenPurpose {
    Initial,
    Prewarm,
    Recovery,
}

#[derive(Debug, Default)]
struct SourceCancellationState {
    cancelled: AtomicBool,
    waker: Mutex<Option<Waker>>,
}

#[derive(Debug, Clone, Default)]
pub struct SourceCancellation {
    state: Arc<SourceCancellationState>,
}

impl SourceCancellation {
    pub fn cancel(&self) {
        self.state.cancelled.store(true, Ordering::Release);
        if let Some(waker) = self
            .state
            .waker
            .lock()
            .expect("cancellation poisoned")
            .take()
        {
            waker.wake();
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.state.cancelled.load(Ordering::Acquire)
    }

    pub fn cancelled(&self) -> SourceCancelled<'_> {
        SourceCancelled { cancellation: self }
    }
}

pub struct SourceCancelled<'a> {
    cancellation: &'a SourceCancellation,
}

impl Future for SourceCancelled<'_> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        if self.cancellation.is_cancelled() {
            return Poll::Ready(());
        }
        *self
            .cancellation
            .state
            .waker
            .lock()
            .expect("cancellation poisoned") = Some(context.waker().clone());
        if self.cancellation.is_cancelled() {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

#[derive(Debug, Clone)]
pub struct SourceOpenRequest {
    pub purpose: SourceOpenPurpose,
    pub deadline: Option<Instant>,
    pub cancellation: SourceCancellation,
}

pub trait SourceFactory: Send + Sync {
    fn descriptor(&self) -> SourceDescriptor;
    fn open(&self, request: SourceOpenRequest) -> SourceOpenFuture<'_>;
}

#[derive(Clone)]
pub struct MemorySourceFactory {
    bytes: Arc<[u8]>,
    descriptor: SourceDescriptor,
}

impl MemorySourceFactory {
    pub fn new(bytes: impl Into<Arc<[u8]>>, descriptor: SourceDescriptor) -> Self {
        Self {
            bytes: bytes.into(),
            descriptor,
        }
    }
}

struct MemoryEncodedSource {
    cursor: Cursor<Arc<[u8]>>,
}

impl Read for MemoryEncodedSource {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        self.cursor.read(output)
    }
}

impl Seek for MemoryEncodedSource {
    fn seek(&mut self, position: std::io::SeekFrom) -> std::io::Result<u64> {
        self.cursor.seek(position)
    }
}

impl EncodedSource for MemoryEncodedSource {
    fn byte_len(&self) -> Option<u64> {
        Some(self.cursor.get_ref().len() as u64)
    }

    fn is_seekable(&self) -> bool {
        true
    }
}

impl SourceFactory for MemorySourceFactory {
    fn descriptor(&self) -> SourceDescriptor {
        self.descriptor.clone()
    }

    fn open(&self, _request: SourceOpenRequest) -> SourceOpenFuture<'_> {
        let bytes = Arc::clone(&self.bytes);
        Box::pin(async move {
            Ok(Box::new(MemoryEncodedSource {
                cursor: Cursor::new(bytes),
            }) as Box<dyn EncodedSource>)
        })
    }
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use super::{
        MemorySourceFactory, SourceCancellation, SourceFactory, SourceOpenPurpose,
        SourceOpenRequest,
    };
    use crate::SourceDescriptor;

    #[tokio::test]
    async fn memory_factory_reopens_from_the_beginning() {
        let factory = MemorySourceFactory::new(
            std::sync::Arc::<[u8]>::from(&b"encoded"[..]),
            SourceDescriptor::default(),
        );
        for _ in 0..2 {
            let mut source = factory
                .open(SourceOpenRequest {
                    purpose: SourceOpenPurpose::Initial,
                    deadline: None,
                    cancellation: SourceCancellation::default(),
                })
                .await
                .unwrap();
            let mut bytes = Vec::new();
            source.read_to_end(&mut bytes).unwrap();
            assert_eq!(bytes, b"encoded");
        }
    }
}
