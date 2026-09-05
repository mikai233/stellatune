//! Keeps synchronous container parsing on an I/O worker. A demuxer may consume
//! half a packet before WouldBlock and cannot generally resume that operation.
//! The actor polls complete decoder operations instead of interrupting parsing.
use std::{
    io::{self, Read, Seek, SeekFrom},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError},
    },
    time::{Duration, Instant},
};
use stellatune_audio_core::{
    decoder::{DecodeStatus, DecodedStreamInfo, DecoderSeekStatus, DecoderStage},
    error::DecodeError,
    format::AudioBlock,
    source::{EncodedSource, MediaHints},
};

enum Command {
    Decode(
        AudioBlock,
        mpsc::Sender<Result<(AudioBlock, DecodeStatus), DecodeError>>,
    ),
    Seek(u64, mpsc::Sender<Result<DecoderSeekStatus, DecodeError>>),
}

type DecodeReply = Receiver<Result<(AudioBlock, DecodeStatus), DecodeError>>;
type SeekReply = Receiver<Result<DecoderSeekStatus, DecodeError>>;

#[derive(Default)]
pub(crate) struct DecoderWorker {
    commands: Option<SyncSender<Command>>,
    cancelled: Arc<AtomicBool>,
    decode: Option<DecodeReply>,
    seek: Option<SeekReply>,
    target: Option<u64>,
}

impl DecoderWorker {
    pub(crate) fn open(
        &mut self,
        mut decoder: Box<dyn DecoderStage>,
        source: Box<dyn EncodedSource>,
        hints: &MediaHints,
    ) -> Result<DecodedStreamInfo, DecodeError> {
        self.reset();
        self.cancelled = Arc::new(AtomicBool::new(false));
        let source = BlockingSource {
            inner: source,
            cancelled: self.cancelled.clone(),
        };
        let cancelled = self.cancelled.clone();
        let hints = hints.clone();
        let (commands, work) = mpsc::sync_channel(2);
        let (opened, ready) = mpsc::channel();
        std::thread::Builder::new()
            .name("stellatune-decoder".into())
            .spawn(move || {
                match decoder.open(Box::new(source), &hints) {
                    Ok(info) => {
                        if opened.send(Ok(info)).is_err() {
                            return;
                        }
                    },
                    Err(error) => {
                        let _ = opened.send(Err(error));
                        return;
                    },
                }
                while !cancelled.load(Ordering::Acquire) {
                    let Ok(command) = work.recv() else {
                        break;
                    };
                    match command {
                        Command::Decode(mut block, reply) => {
                            let result = decoder.decode(&mut block).map(|status| (block, status));
                            let _ = reply.send(result);
                        },
                        Command::Seek(target, reply) => {
                            let _ = reply.send(decoder.start_seek(target));
                        },
                    }
                }
            })
            .map_err(DecodeError::Io)?;
        self.commands = Some(commands);
        match ready.recv_timeout(Duration::from_secs(30)) {
            Ok(Ok(info)) => Ok(info),
            Ok(Err(error)) => {
                self.reset();
                Err(error)
            },
            Err(error) => {
                self.reset();
                Err(failed(error.to_string()))
            },
        }
    }

    pub(crate) fn decode(&mut self, output: &mut AudioBlock) -> Result<DecodeStatus, DecodeError> {
        if self.target.is_some() {
            return Ok(DecodeStatus::Pending);
        }
        if self.decode.is_none() {
            let (tx, rx) = mpsc::channel();
            let block = std::mem::replace(output, AudioBlock::new(output.format));
            match self
                .commands
                .as_ref()
                .ok_or_else(|| failed("decoder is closed"))?
                .try_send(Command::Decode(block, tx))
            {
                Ok(()) => self.decode = Some(rx),
                Err(TrySendError::Full(Command::Decode(block, _))) => {
                    *output = block;
                    return Ok(DecodeStatus::Pending);
                },
                Err(_) => return Err(failed("decoder worker closed")),
            }
        }
        match self.decode.as_ref().unwrap().try_recv() {
            Ok(result) => {
                self.decode = None;
                let (block, status) = result?;
                *output = block;
                Ok(status)
            },
            Err(TryRecvError::Empty) => Ok(DecodeStatus::Pending),
            Err(TryRecvError::Disconnected) => {
                self.decode = None;
                Err(failed("decoder worker closed"))
            },
        }
    }

    pub(crate) fn start_seek(&mut self, target: u64) -> Result<DecoderSeekStatus, DecodeError> {
        self.decode = None;
        self.seek = None;
        self.target = Some(target);
        self.continue_seek()
    }

    pub(crate) fn continue_seek(&mut self) -> Result<DecoderSeekStatus, DecodeError> {
        let target = self.target.ok_or(DecodeError::Unsupported)?;
        if self.seek.is_none() {
            let (tx, rx) = mpsc::channel();
            match self
                .commands
                .as_ref()
                .ok_or_else(|| failed("decoder is closed"))?
                .try_send(Command::Seek(target, tx))
            {
                Ok(()) => self.seek = Some(rx),
                Err(TrySendError::Full(_)) => return Ok(DecoderSeekStatus::Pending),
                Err(_) => return Err(failed("decoder worker closed")),
            }
        }
        match self.seek.as_ref().unwrap().try_recv() {
            Ok(result) => {
                self.seek = None;
                if !matches!(result, Ok(DecoderSeekStatus::Pending)) {
                    self.target = None;
                }
                result
            },
            Err(TryRecvError::Empty) => Ok(DecoderSeekStatus::Pending),
            Err(TryRecvError::Disconnected) => {
                self.seek = None;
                self.target = None;
                Err(failed("decoder worker closed"))
            },
        }
    }

    pub(crate) fn reset(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        self.commands = None;
        self.decode = None;
        self.seek = None;
        self.target = None;
    }
}
impl Drop for DecoderWorker {
    fn drop(&mut self) {
        self.reset();
    }
}
fn failed(message: impl Into<String>) -> DecodeError {
    DecodeError::Failed {
        message: message.into(),
    }
}

struct BlockingSource {
    inner: Box<dyn EncodedSource>,
    cancelled: Arc<AtomicBool>,
}
impl Read for BlockingSource {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if self.cancelled.load(Ordering::Acquire) {
                return Err(io::Error::other("decoder cancelled"));
            }
            match self.inner.read(output) {
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        return Err(io::Error::new(
                            io::ErrorKind::TimedOut,
                            "source read timed out",
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(2));
                },
                result => return result,
            }
        }
    }
}
impl Seek for BlockingSource {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.inner.seek(position)
    }
}
impl EncodedSource for BlockingSource {
    fn byte_len(&self) -> Option<u64> {
        self.inner.byte_len()
    }
    fn is_seekable(&self) -> bool {
        self.inner.is_seekable()
    }
}
