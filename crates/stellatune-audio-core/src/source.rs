//! Encoded media sources and asynchronous source opening.
//!
//! A [`SourceFactory`](crate::source::SourceFactory) performs potentially slow
//! acquisition asynchronously. It produces an
//! [`EncodedSource`](crate::source::EncodedSource) whose [`Read`](std::io::Read)
//! and [`Seek`](std::io::Seek) operations are synchronous and must remain
//! bounded. Streaming adapters represent temporary input starvation with
//! `std::io::ErrorKind::WouldBlock` instead of waiting in the playback data
//! path.

use std::future::Future;
use std::io::{Cursor, Read, Seek};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::time::Instant;

use crate::error::SourceError;

/// Hints used to select and initialize a decoder.
///
/// Hints describe the encoded representation; none of them is authoritative,
/// and a decoder may still inspect the byte stream.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MediaHints {
    /// A filename extension without a required leading dot.
    pub extension: Option<String>,
    /// The encoded stream's media type, when known.
    pub mime_type: Option<String>,
    /// The total encoded length in bytes, when known.
    pub content_length: Option<u64>,
    /// An adapter-specific container name or format hint.
    pub container_hint: Option<String>,
}

/// Operations that an encoded source can support across openings.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SourceCapabilities {
    /// Whether an opened source supports meaningful byte seeks.
    pub byte_seekable: bool,
    /// Whether the factory can open the same logical media again.
    pub reopenable: bool,
    /// Whether the source represents a live stream without a fixed end.
    pub live: bool,
}

/// Decoder-selection metadata and capabilities for a source factory.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceDescriptor {
    /// Hints describing the encoded media representation.
    pub media: MediaHints,
    /// Operations supported by the source.
    pub capabilities: SourceCapabilities,
}

/// A synchronously readable encoded byte stream.
///
/// Implementations that are not byte-seekable still implement [`Seek`] but
/// return an appropriate I/O error for unsupported seek operations. The
/// [`Self::is_seekable`] result must agree with the factory's
/// [`SourceCapabilities::byte_seekable`] declaration.
pub trait EncodedSource: Read + Seek + Send {
    /// Returns the total encoded length in bytes, if it is known.
    fn byte_len(&self) -> Option<u64>;
    /// Returns whether byte-offset seeking is supported.
    fn is_seekable(&self) -> bool;
}

/// The future returned by [`SourceFactory::open`].
pub type SourceOpenFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Box<dyn EncodedSource>, SourceError>> + Send + 'a>>;

/// The runtime operation for which a source is being opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceOpenPurpose {
    /// Opens the current item for its first preparation.
    Initial,
    /// Opens a queued item before it becomes current.
    Prewarm,
    /// Reopens the current item after a recoverable failure.
    Recovery,
}

#[derive(Debug, Default)]
struct SourceCancellationState {
    cancelled: AtomicBool,
    waker: Mutex<Option<Waker>>,
}

/// A cloneable, sticky cancellation signal for a source-opening operation.
///
/// Cancelling any clone cancels all clones. Cancellation is cooperative: a
/// [`SourceFactory`] must observe the signal or await [`Self::cancelled`] while
/// performing external I/O.
#[derive(Debug, Clone, Default)]
pub struct SourceCancellation {
    state: Arc<SourceCancellationState>,
}

impl SourceCancellation {
    /// Marks the operation as cancelled and wakes its cancellation waiter.
    ///
    /// Calling this method more than once has no additional effect.
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

    /// Returns whether cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.state.cancelled.load(Ordering::Acquire)
    }

    /// Returns a future that completes when cancellation is requested.
    ///
    /// Source-opening code should await one cancellation future per request.
    pub fn cancelled(&self) -> SourceCancelled<'_> {
        SourceCancelled { cancellation: self }
    }
}

/// A future that resolves after its [`SourceCancellation`] is cancelled.
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

/// Context supplied to a [`SourceFactory`] for one open attempt.
#[derive(Debug, Clone)]
pub struct SourceOpenRequest {
    /// The operation that requested this source.
    pub purpose: SourceOpenPurpose,
    /// The caller's absolute deadline, when one has been assigned.
    ///
    /// Factories should avoid beginning work that cannot finish before this
    /// instant. The runtime also enforces its own preparation deadline.
    pub deadline: Option<Instant>,
    /// The cooperative cancellation signal for this attempt.
    pub cancellation: SourceCancellation,
}

/// Opens independent encoded streams for a logical playback source.
///
/// Implementations declare stable hints and capabilities through
/// [`Self::descriptor`]. When `reopenable` is true, each successful call to
/// [`Self::open`] must return a fresh stream positioned at its beginning.
pub trait SourceFactory: Send + Sync {
    /// Returns the source's media hints and capabilities.
    fn descriptor(&self) -> SourceDescriptor;
    /// Begins opening an encoded stream for one playback operation.
    ///
    /// # Errors
    ///
    /// The future returns [`SourceError::Cancelled`] when cooperative
    /// cancellation wins, [`SourceError::Unsupported`] when the requested
    /// operation cannot be provided, or another [`SourceError`] for acquisition
    /// and I/O failures.
    fn open(&self, request: SourceOpenRequest) -> SourceOpenFuture<'_>;
}

/// A reopenable source factory backed by immutable memory.
///
/// Each opening starts at byte offset zero. Opening is immediate and therefore
/// does not need to consult the request deadline or cancellation signal.
#[derive(Clone)]
pub struct MemorySourceFactory {
    bytes: Arc<[u8]>,
    descriptor: SourceDescriptor,
}

impl MemorySourceFactory {
    /// Creates a memory source from encoded bytes and their descriptor.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::io::Read;
    /// use std::sync::Arc;
    /// use stellatune_audio_core::source::{
    ///     MemorySourceFactory, SourceCancellation, SourceDescriptor,
    ///     SourceFactory, SourceOpenPurpose, SourceOpenRequest,
    /// };
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), stellatune_audio_core::error::SourceError> {
    /// let factory = MemorySourceFactory::new(
    ///     Arc::<[u8]>::from(&b"encoded audio"[..]),
    ///     SourceDescriptor::default(),
    /// );
    /// let mut source = factory.open(SourceOpenRequest {
    ///     purpose: SourceOpenPurpose::Initial,
    ///     deadline: None,
    ///     cancellation: SourceCancellation::default(),
    /// }).await?;
    ///
    /// let mut bytes = Vec::new();
    /// source.read_to_end(&mut bytes)?;
    /// assert_eq!(bytes, b"encoded audio");
    /// # Ok(())
    /// # }
    /// ```
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

    use super::SourceDescriptor;
    use super::{
        MemorySourceFactory, SourceCancellation, SourceFactory, SourceOpenPurpose,
        SourceOpenRequest,
    };

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
