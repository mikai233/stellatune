use std::collections::VecDeque;
use std::sync::{Mutex, MutexGuard};

/// Ring buffer abstraction used by the future decode -> output pipeline.
///
/// For real-time audio, this should become a lock-free SPSC ring buffer.
/// The current implementation is a simple, correctness-first placeholder.
pub trait RingBuffer<T>: Send {
    fn push_slice(&self, items: &[T]) -> usize
    where
        T: Copy;

    fn pop_slice(&self, out: &mut [T]) -> usize
    where
        T: Copy;

    fn len(&self) -> usize;
    fn capacity(&self) -> usize;
}

pub struct MutexRingBuffer<T> {
    capacity: usize,
    inner: Mutex<VecDeque<T>>,
}

impl<T> MutexRingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            inner: Mutex::new(VecDeque::with_capacity(capacity)),
        }
    }

    fn guard(&self) -> MutexGuard<'_, VecDeque<T>> {
        self.inner.lock().expect("ring buffer mutex poisoned")
    }
}

impl<T: Send> RingBuffer<T> for MutexRingBuffer<T> {
    fn push_slice(&self, items: &[T]) -> usize
    where
        T: Copy,
    {
        let mut inner = self.guard();
        let available = self.capacity.saturating_sub(inner.len());
        let to_write = available.min(items.len());
        inner.extend(items.iter().copied().take(to_write));
        to_write
    }

    fn pop_slice(&self, out: &mut [T]) -> usize
    where
        T: Copy,
    {
        let mut inner = self.guard();
        let to_read = inner.len().min(out.len());
        for slot in out.iter_mut().take(to_read) {
            *slot = inner.pop_front().expect("len checked");
        }
        to_read
    }

    fn len(&self) -> usize {
        self.guard().len()
    }

    fn capacity(&self) -> usize {
        self.capacity
    }
}
