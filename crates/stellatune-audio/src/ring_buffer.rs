use ringbuf::traits::{Consumer as _, Observer as _, Producer as _, Split as _};
use ringbuf::{HeapCons, HeapProd, HeapRb};

/// Ring buffer abstraction used by the future decode -> output pipeline.
///
/// This is a single-producer/single-consumer (SPSC) ring buffer, intended for:
/// - decode thread: writes samples
/// - audio output callback/thread: reads samples
///
/// The producer and consumer handles must be moved to their respective threads.
pub struct RingBufferProducer<T> {
    inner: HeapProd<T>,
}

pub struct RingBufferConsumer<T> {
    inner: HeapCons<T>,
}

pub fn new_ring_buffer<T>(capacity: usize) -> (RingBufferProducer<T>, RingBufferConsumer<T>) {
    let rb = HeapRb::<T>::new(capacity);
    let (producer, consumer) = rb.split();
    (
        RingBufferProducer { inner: producer },
        RingBufferConsumer { inner: consumer },
    )
}

impl<T> RingBufferProducer<T> {
    pub fn push_slice(&mut self, items: &[T]) -> usize
    where
        T: Copy,
    {
        self.inner.push_slice(items)
    }

    pub fn len(&self) -> usize {
        self.inner.occupied_len()
    }

    pub fn capacity(&self) -> usize {
        self.inner.capacity().get()
    }
}

impl<T> RingBufferConsumer<T> {
    pub fn pop_slice(&mut self, out: &mut [T]) -> usize
    where
        T: Copy,
    {
        self.inner.pop_slice(out)
    }

    pub fn len(&self) -> usize {
        self.inner.occupied_len()
    }

    pub fn capacity(&self) -> usize {
        self.inner.capacity().get()
    }
}

impl RingBufferConsumer<f32> {
    pub fn pop_sample(&mut self) -> Option<f32> {
        let mut tmp = [0.0f32];
        (self.pop_slice(&mut tmp) == 1).then_some(tmp[0])
    }
}
