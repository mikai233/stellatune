//! Decoder read-ahead bounded by media frames and retained PCM allocation.
//! Dropping the reader releases a blocked producer, including during seek/reset.
use std::{
    collections::VecDeque,
    sync::{Arc, Condvar, Mutex, mpsc::TryRecvError},
};
use stellatune_audio_core::{
    buffering::{MAX_BLOCK_BYTES, MAX_BUFFER_BYTES},
    decoder::DecodeStatus,
    error::DecodeError,
    format::AudioBlock,
};

pub(crate) type Reply = Result<(AudioBlock, DecodeStatus), DecodeError>;
#[derive(Default)]
struct State {
    queue: VecDeque<Reply>,
    frames: usize,
    bytes: usize,
    closed: bool,
    producer_done: bool,
}
struct Shared {
    state: Mutex<State>,
    room: Condvar,
    target_frames: usize,
}
pub(crate) struct Sender(Arc<Shared>);
pub(crate) struct Receiver(Arc<Shared>);

pub(crate) fn channel(target_frames: usize) -> (Sender, Receiver) {
    let shared = Arc::new(Shared {
        state: Mutex::default(),
        room: Condvar::new(),
        target_frames: target_frames.max(1),
    });
    (Sender(shared.clone()), Receiver(shared))
}
fn size(reply: &Reply) -> (usize, usize) {
    reply
        .as_ref()
        .map(|(block, _)| (block.frames(), block.samples.capacity() * size_of::<f32>()))
        .unwrap_or_default()
}
impl Sender {
    pub(crate) fn send(&self, mut reply: Reply) -> Result<(), ()> {
        if size(&reply).1 > MAX_BLOCK_BYTES {
            reply = Err(DecodeError::Failed {
                message: "decoder PCM block exceeds memory limit".into(),
            });
        }
        let (frames, bytes) = size(&reply);
        let mut state = self.0.state.lock().unwrap();
        while !state.closed
            && (state.frames >= self.0.target_frames
                || state.bytes + bytes > MAX_BUFFER_BYTES
                || state.queue.len() >= 4096)
        {
            state = self.0.room.wait(state).unwrap();
        }
        if state.closed {
            return Err(());
        }
        state.frames += frames;
        state.bytes += bytes;
        state.queue.push_back(reply);
        Ok(())
    }
}
impl Drop for Sender {
    fn drop(&mut self) {
        self.0.state.lock().unwrap().producer_done = true;
    }
}
impl Receiver {
    pub(crate) fn try_recv(&self) -> Result<Reply, TryRecvError> {
        let mut state = self.0.state.lock().unwrap();
        let Some(reply) = state.queue.pop_front() else {
            return Err(if state.producer_done {
                TryRecvError::Disconnected
            } else {
                TryRecvError::Empty
            });
        };
        let (frames, bytes) = size(&reply);
        state.frames -= frames;
        state.bytes -= bytes;
        self.0.room.notify_one();
        Ok(reply)
    }
}
impl Drop for Receiver {
    fn drop(&mut self) {
        let mut state = self.0.state.lock().unwrap();
        state.closed = true;
        state.queue.clear();
        self.0.room.notify_one();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::mpsc, time::Duration};
    use stellatune_audio_core::{
        buffering::frames_for_ms,
        format::{ChannelLayout, PcmFormat},
    };

    #[test]
    fn variable_packet_sizes_obey_time_budget_and_reader_drop_releases_producer() {
        for (rate, packet_frames) in [(8000, 200), (192000, 2000)] {
            let format = PcmFormat {
                sample_rate: rate,
                channel_layout: ChannelLayout::STEREO,
            };
            let target = frames_for_ms(format, 100);
            let packets = target.div_ceil(packet_frames);
            let (sender, receiver) = channel(target);
            let (sent, observed) = mpsc::channel();
            let (done, finished) = mpsc::channel();
            let thread = std::thread::spawn(move || {
                loop {
                    let mut block = AudioBlock::new(format);
                    block.samples = vec![0.0; packet_frames * 2];
                    if sender
                        .send(Ok((
                            block,
                            DecodeStatus::Produced {
                                frames: packet_frames,
                            },
                        )))
                        .is_err()
                    {
                        break;
                    }
                    sent.send(()).unwrap();
                }
                done.send(()).unwrap();
            });
            for _ in 0..packets {
                observed.recv_timeout(Duration::from_secs(1)).unwrap();
            }
            assert!(
                observed.recv_timeout(Duration::from_millis(20)).is_err(),
                "stalled consumer must bound decoded duration"
            );
            receiver.try_recv().unwrap().unwrap();
            observed.recv_timeout(Duration::from_secs(1)).unwrap();
            assert!(observed.recv_timeout(Duration::from_millis(20)).is_err());
            drop(receiver);
            finished.recv_timeout(Duration::from_secs(1)).unwrap();
            thread.join().unwrap();
        }
    }

    #[test]
    fn oversized_pcm_returns_an_error_instead_of_filling_the_queue() {
        let format = PcmFormat {
            sample_rate: 48000,
            channel_layout: ChannelLayout::STEREO,
        };
        let (sender, receiver) = channel(4800);
        let mut block = AudioBlock::new(format);
        block.samples = vec![0.0; MAX_BLOCK_BYTES / size_of::<f32>() + 2];
        sender
            .send(Ok((block, DecodeStatus::Produced { frames: 1 })))
            .unwrap();
        assert!(matches!(
            receiver.try_recv().unwrap(),
            Err(DecodeError::Failed { .. })
        ));
    }
}
