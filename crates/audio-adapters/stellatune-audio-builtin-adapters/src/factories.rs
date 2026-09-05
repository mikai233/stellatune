use std::collections::{BTreeMap, VecDeque};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver as StdReceiver, SyncSender, TryRecvError, sync_channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use stellatune_audio_core::{
    decoder::{
        DecodeStatus, DecodedStreamInfo, DecoderDescriptor, DecoderFactory, DecoderSeekStatus,
        DecoderStage, GaplessTrimSpec, SeekResult,
    },
    error::{DecodeError, FactoryError, SourceError},
    format::{AudioBlock, PcmFormat},
    sink::{OutputCompatibilityKey, SinkFactory, SinkStage},
    source::{
        EncodedSource, MediaHints, SourceCancellation, SourceCapabilities, SourceDescriptor,
        SourceFactory, SourceOpenFuture, SourceOpenRequest,
    },
    stage::StageId,
};
use symphonia::core::io::MediaSource;

use crate::builtin_decoder::{
    BuiltinDecoder, SOURCE_IO_ERROR_PREFIX, SOURCE_PENDING_ERROR,
    builtin_decoder_supported_extensions,
};
use crate::device_sink::{DeviceSinkControl, DeviceSinkStage, OutputBackend};

#[derive(Debug, Clone)]
pub struct FileSourceFactory {
    path: PathBuf,
    descriptor: SourceDescriptor,
}

impl FileSourceFactory {
    pub fn new(path: PathBuf, mut media: MediaHints) -> Result<Self, SourceError> {
        if path.as_os_str().is_empty() {
            return Err(SourceError::Failed {
                message: "file path cannot be empty".to_owned(),
            });
        }
        if media.extension.is_none() {
            media.extension = path
                .extension()
                .and_then(|value| value.to_str())
                .map(|value| value.to_ascii_lowercase());
        }
        if media.content_length.is_none() {
            media.content_length = std::fs::metadata(&path).ok().map(|metadata| metadata.len());
        }
        Ok(Self {
            path,
            descriptor: SourceDescriptor {
                media,
                capabilities: SourceCapabilities {
                    byte_seekable: true,
                    reopenable: true,
                    live: false,
                },
            },
        })
    }
}

struct FileEncodedSource {
    file: File,
    len: u64,
}

impl Read for FileEncodedSource {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        self.file.read(output)
    }
}

impl Seek for FileEncodedSource {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.file.seek(position)
    }
}

impl EncodedSource for FileEncodedSource {
    fn byte_len(&self) -> Option<u64> {
        Some(self.len)
    }

    fn is_seekable(&self) -> bool {
        true
    }
}

impl SourceFactory for FileSourceFactory {
    fn descriptor(&self) -> SourceDescriptor {
        self.descriptor.clone()
    }

    fn open(&self, _request: SourceOpenRequest) -> SourceOpenFuture<'_> {
        let path = self.path.clone();
        Box::pin(async move {
            let file = File::open(&path)?;
            let len = file.metadata()?.len();
            Ok(Box::new(FileEncodedSource { file, len }) as Box<dyn EncodedSource>)
        })
    }
}

#[derive(Clone)]
pub struct HttpSourceFactory {
    url: String,
    headers: BTreeMap<String, String>,
    descriptor: SourceDescriptor,
}

impl std::fmt::Debug for HttpSourceFactory {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HttpSourceFactory")
            .field("url", &"<redacted HTTP URL>")
            .field("header_count", &self.headers.len())
            .field("descriptor", &self.descriptor)
            .finish()
    }
}

impl HttpSourceFactory {
    pub fn new(
        url: String,
        headers: BTreeMap<String, String>,
        media: MediaHints,
        capabilities: SourceCapabilities,
    ) -> Result<Self, SourceError> {
        let parsed = url::Url::parse(url.trim()).map_err(|error| SourceError::Failed {
            message: format!("invalid HTTP URL: {error}"),
        })?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err(SourceError::Failed {
                message: "HTTP source URL must use http or https".to_owned(),
            });
        }
        for (name, value) in &headers {
            reqwest::header::HeaderName::from_bytes(name.as_bytes()).map_err(|error| {
                SourceError::Failed {
                    message: format!("invalid HTTP header name: {error}"),
                }
            })?;
            reqwest::header::HeaderValue::from_str(value).map_err(|error| SourceError::Failed {
                message: format!("invalid HTTP header value: {error}"),
            })?;
        }
        Ok(Self {
            url: parsed.to_string(),
            headers,
            descriptor: SourceDescriptor {
                media,
                capabilities,
            },
        })
    }
}

struct HttpEncodedSource {
    client: reqwest::Client,
    url: String,
    headers: BTreeMap<String, String>,
    position: u64,
    total_size: Option<u64>,
    seekable: bool,
    current: std::io::Cursor<Arc<[u8]>>,
    cache: VecDeque<(u64, Arc<[u8]>)>,
    cache_bytes: usize,
    feeder_position: u64,
    receiver: StdReceiver<HttpFeederMessage>,
    feeder_cancellation: SourceCancellation,
    eof: bool,
}

enum HttpFeederMessage {
    Data(Vec<u8>),
    Eof,
    Failed(String),
}

const HTTP_INITIAL_READ_BYTES: usize = 64 * 1024;
const HTTP_FEEDER_CHUNK_BYTES: usize = 32 * 1024;
const HTTP_FEEDER_CHUNKS: usize = 8;
const HTTP_SEEK_CACHE_BYTES: usize = 2 * 1024 * 1024;

impl HttpEncodedSource {
    async fn open(
        url: String,
        headers: BTreeMap<String, String>,
        open_cancellation: SourceCancellation,
    ) -> Result<Self, SourceError> {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .build()
            .map_err(|error| SourceError::Failed {
                message: error.to_string(),
            })?;
        let mut request = client.get(&url);
        for (name, value) in &headers {
            request = request.header(name, value);
        }
        let mut response = tokio::select! {
            result = request.send() => result
                .and_then(reqwest::Response::error_for_status)
                .map_err(|error| SourceError::Failed { message: error.to_string() })?,
            () = open_cancellation.cancelled() => return Err(SourceError::Cancelled),
        };
        let total_size = response.content_length();
        let seekable = response
            .headers()
            .get(reqwest::header::ACCEPT_RANGES)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.eq_ignore_ascii_case("bytes"));
        // Read only the first available network chunk before handing the response
        // to the bounded feeder. `read_to_end` on a limited reader still waits for
        // the full limit (or EOF), which can stall preparation indefinitely for a
        // slow or live stream.
        let mut initial = tokio::select! {
            result = response.chunk() => result
                .map_err(|error| SourceError::Failed { message: error.to_string() })?
                .map_or_else(Vec::new, |chunk| chunk.to_vec()),
            () = open_cancellation.cancelled() => return Err(SourceError::Cancelled),
        };
        let prefetched_remainder = if initial.len() > HTTP_INITIAL_READ_BYTES {
            initial.split_off(HTTP_INITIAL_READ_BYTES)
        } else {
            Vec::new()
        };
        let (sender, receiver) = sync_channel(HTTP_FEEDER_CHUNKS);
        let feeder_cancellation = SourceCancellation::default();
        spawn_async_http_feeder(
            response,
            prefetched_remainder,
            sender,
            feeder_cancellation.clone(),
        );
        let initial: Arc<[u8]> = initial.into();
        Ok(Self {
            cache: VecDeque::from([(0, initial.clone())]),
            cache_bytes: initial.len(),
            feeder_position: initial.len() as u64,
            client,
            url,
            headers,
            position: 0,
            total_size,
            seekable,
            current: std::io::Cursor::new(initial),
            receiver,
            feeder_cancellation,
            eof: false,
        })
    }

    // Retrying a demuxer seek may revisit several byte offsets. Reuse bounded
    // encoded chunks so retries make progress instead of restarting every range.
    fn use_cached(&mut self, offset: u64) -> bool {
        let Some((start, chunk)) = self
            .cache
            .iter()
            .rev()
            .find(|(start, chunk)| offset >= *start && offset - *start < chunk.len() as u64)
        else {
            return false;
        };
        self.current = std::io::Cursor::new(chunk.clone());
        self.current.set_position(offset - *start);
        true
    }

    fn retain_chunk(&mut self, start: u64, chunk: Arc<[u8]>) {
        self.cache_bytes += chunk.len();
        self.cache.push_back((start, chunk));
        while self.cache_bytes > HTTP_SEEK_CACHE_BYTES {
            if let Some((_, old)) = self.cache.pop_front() {
                self.cache_bytes -= old.len();
            }
        }
    }

    fn restart_at(&mut self, offset: u64) {
        self.feeder_cancellation.cancel();
        self.feeder_cancellation = SourceCancellation::default();
        let client = self.client.clone();
        let url = self.url.clone();
        let headers = self.headers.clone();
        let (sender, receiver) = sync_channel(HTTP_FEEDER_CHUNKS);
        spawn_async_http_range(
            client,
            url,
            headers,
            offset,
            sender,
            self.feeder_cancellation.clone(),
        );
        self.current = std::io::Cursor::new(Arc::from([]));
        self.feeder_position = offset;
        self.receiver = receiver;
        self.eof = false;
    }
}

fn spawn_async_http_range(
    client: reqwest::Client,
    url: String,
    headers: BTreeMap<String, String>,
    offset: u64,
    sender: SyncSender<HttpFeederMessage>,
    cancellation: SourceCancellation,
) {
    let _ = std::thread::Builder::new()
        .name("stellatune-http-source-range".to_owned())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    let _ = sender.send(HttpFeederMessage::Failed(error.to_string()));
                    return;
                },
            };
            runtime.block_on(async move {
                let mut request = client.get(url);
                for (name, value) in headers {
                    request = request.header(name, value);
                }
                request = request.header(reqwest::header::RANGE, format!("bytes={offset}-"));
                let response = tokio::select! {
                    result = request.send() => match result.and_then(reqwest::Response::error_for_status) {
                        Ok(response) => response,
                        Err(error) => {
                            let _ = sender.send(HttpFeederMessage::Failed(error.to_string()));
                            return;
                        },
                    },
                    () = cancellation.cancelled() => return,
                };
                if response.status() != reqwest::StatusCode::PARTIAL_CONTENT {
                    let _ = sender.send(HttpFeederMessage::Failed(
                        "HTTP range seek is unsupported".to_owned(),
                    ));
                    return;
                }
                feed_async_http_response(response, sender, cancellation).await;
            });
        });
}

fn spawn_async_http_feeder(
    response: reqwest::Response,
    prefetched: Vec<u8>,
    sender: SyncSender<HttpFeederMessage>,
    cancellation: SourceCancellation,
) {
    let _ = std::thread::Builder::new()
        .name("stellatune-http-source-feeder".to_owned())
        .spawn(move || {
            for chunk in prefetched.chunks(HTTP_FEEDER_CHUNK_BYTES) {
                if sender
                    .send(HttpFeederMessage::Data(chunk.to_vec()))
                    .is_err()
                {
                    return;
                }
            }
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    let _ = sender.send(HttpFeederMessage::Failed(error.to_string()));
                    return;
                },
            };
            runtime.block_on(feed_async_http_response(response, sender, cancellation));
        });
}

async fn feed_async_http_response(
    mut response: reqwest::Response,
    sender: SyncSender<HttpFeederMessage>,
    cancellation: SourceCancellation,
) {
    loop {
        let chunk = tokio::select! {
            result = response.chunk() => result,
            () = cancellation.cancelled() => break,
        };
        match chunk {
            Ok(Some(chunk)) => {
                for part in chunk.chunks(HTTP_FEEDER_CHUNK_BYTES) {
                    if cancellation.is_cancelled()
                        || sender.send(HttpFeederMessage::Data(part.to_vec())).is_err()
                    {
                        return;
                    }
                }
            },
            Ok(None) => {
                let _ = sender.send(HttpFeederMessage::Eof);
                break;
            },
            Err(error) => {
                let _ = sender.send(HttpFeederMessage::Failed(error.to_string()));
                break;
            },
        }
    }
}

impl Read for HttpEncodedSource {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        loop {
            let read = self.current.read(output)?;
            if read > 0 {
                self.position = self.position.saturating_add(read as u64);
                return Ok(read);
            }
            if self
                .total_size
                .is_some_and(|length| self.position >= length)
            {
                return Ok(0);
            }
            if self.use_cached(self.position) {
                continue;
            }
            if self.position != self.feeder_position {
                self.restart_at(self.position);
            }
            if self.eof {
                return Ok(0);
            }
            match self.receiver.try_recv() {
                Ok(HttpFeederMessage::Data(chunk)) => {
                    let chunk: Arc<[u8]> = chunk.into();
                    self.retain_chunk(self.feeder_position, chunk.clone());
                    self.feeder_position += chunk.len() as u64;
                    self.current = std::io::Cursor::new(chunk);
                },
                Ok(HttpFeederMessage::Eof) | Err(TryRecvError::Disconnected) => {
                    self.eof = true;
                },
                Ok(HttpFeederMessage::Failed(message)) => {
                    return Err(std::io::Error::other(message));
                },
                Err(TryRecvError::Empty) => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::WouldBlock,
                        "HTTP feeder buffer is temporarily empty",
                    ));
                },
            }
        }
    }
}

impl Seek for HttpEncodedSource {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        let target = match position {
            SeekFrom::Start(value) => value,
            SeekFrom::Current(delta) => {
                self.position.checked_add_signed(delta).ok_or_else(|| {
                    std::io::Error::new(std::io::ErrorKind::InvalidInput, "seek overflow")
                })?
            },
            SeekFrom::End(delta) => self
                .total_size
                .ok_or_else(|| {
                    std::io::Error::new(std::io::ErrorKind::Unsupported, "unknown HTTP length")
                })?
                .checked_add_signed(delta)
                .ok_or_else(|| {
                    std::io::Error::new(std::io::ErrorKind::InvalidInput, "seek overflow")
                })?,
        };
        if target == self.position {
            return Ok(target);
        }
        if target != 0 && !self.seekable {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "HTTP range seek is unsupported",
            ));
        }
        if !self.use_cached(target) {
            // An EOF length query must not cancel an in-flight range.
            if Some(target) == self.total_size || target == self.feeder_position {
                self.current = std::io::Cursor::new(Arc::from([]));
            } else {
                self.restart_at(target);
            }
        }
        self.position = target;
        Ok(target)
    }
}

impl EncodedSource for HttpEncodedSource {
    fn byte_len(&self) -> Option<u64> {
        self.total_size
    }

    fn is_seekable(&self) -> bool {
        self.seekable
    }
}

impl Drop for HttpEncodedSource {
    fn drop(&mut self) {
        self.feeder_cancellation.cancel();
    }
}

impl SourceFactory for HttpSourceFactory {
    fn descriptor(&self) -> SourceDescriptor {
        self.descriptor.clone()
    }

    fn open(&self, request: SourceOpenRequest) -> SourceOpenFuture<'_> {
        let url = self.url.clone();
        let headers = self.headers.clone();
        Box::pin(async move {
            let mut source = HttpEncodedSource::open(url, headers, request.cancellation).await?;
            source.seekable &= self.descriptor.capabilities.byte_seekable;
            Ok(Box::new(source) as Box<dyn EncodedSource>)
        })
    }
}

/// Opens the generic decoder over an already acquired encoded source.
/// Blocking consumers must provide a source whose reads wait for input.
pub fn open_builtin_decoder(
    source: Box<dyn EncodedSource>,
    hint_extension: &str,
) -> Result<crate::builtin_decoder::BuiltinDecoder, String> {
    BuiltinDecoder::open_source(
        Box::new(SymphoniaMediaSource(Mutex::new(source))),
        hint_extension,
    )
}

struct SymphoniaMediaSource(Mutex<Box<dyn EncodedSource>>);

impl Read for SymphoniaMediaSource {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        self.0
            .get_mut()
            .map_err(|_| std::io::Error::other("encoded source lock poisoned"))?
            .read(output)
    }
}

impl Seek for SymphoniaMediaSource {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.0
            .get_mut()
            .map_err(|_| std::io::Error::other("encoded source lock poisoned"))?
            .seek(position)
    }
}

impl MediaSource for SymphoniaMediaSource {
    fn is_seekable(&self) -> bool {
        self.0
            .lock()
            .map(|source| source.is_seekable())
            .unwrap_or(false)
    }

    fn byte_len(&self) -> Option<u64> {
        self.0.lock().ok().and_then(|source| source.byte_len())
    }
}

pub struct SymphoniaDecoderFactory {
    descriptor: DecoderDescriptor,
}

impl SymphoniaDecoderFactory {
    pub fn new() -> Self {
        Self {
            descriptor: DecoderDescriptor {
                id: StageId::new("builtin.decoder.symphonia").unwrap(),
                priority: 100,
                extensions: builtin_decoder_supported_extensions(),
                mime_types: vec![
                    "audio/mpeg".to_owned(),
                    "audio/flac".to_owned(),
                    "audio/wav".to_owned(),
                    "audio/ogg".to_owned(),
                    "audio/mp4".to_owned(),
                ],
            },
        }
    }
}

impl Default for SymphoniaDecoderFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl DecoderFactory for SymphoniaDecoderFactory {
    fn descriptor(&self) -> &DecoderDescriptor {
        &self.descriptor
    }

    fn create(&self) -> Result<Box<dyn DecoderStage>, FactoryError> {
        Ok(Box::new(AsyncSymphoniaDecoder::default()))
    }
}

#[derive(Default)]
struct AsyncSymphoniaDecoder(crate::decoder_worker::DecoderWorker);
impl DecoderStage for AsyncSymphoniaDecoder {
    fn open(
        &mut self,
        source: Box<dyn EncodedSource>,
        hints: &MediaHints,
    ) -> Result<DecodedStreamInfo, DecodeError> {
        self.0.open(
            Box::new(SymphoniaDecoderStage {
                decoder: None,
                pending_seek_frame: None,
            }),
            source,
            hints,
        )
    }
    fn decode(&mut self, output: &mut AudioBlock) -> Result<DecodeStatus, DecodeError> {
        self.0.decode(output)
    }
    fn start_seek(&mut self, target: u64) -> Result<DecoderSeekStatus, DecodeError> {
        self.0.start_seek(target)
    }
    fn continue_seek(&mut self) -> Result<DecoderSeekStatus, DecodeError> {
        self.0.continue_seek()
    }
    fn reset(&mut self) {
        self.0.reset();
    }
}

struct SymphoniaDecoderStage {
    decoder: Option<BuiltinDecoder>,
    pending_seek_frame: Option<u64>,
}

impl DecoderStage for SymphoniaDecoderStage {
    fn open(
        &mut self,
        source: Box<dyn EncodedSource>,
        hints: &MediaHints,
    ) -> Result<DecodedStreamInfo, DecodeError> {
        let extension = hints.extension.as_deref().unwrap_or_default();
        let decoder = open_builtin_decoder(source, extension)
            .map_err(|message| DecodeError::Failed { message })?;
        let spec = decoder.spec();
        let duration_frames = decoder
            .duration_ms_hint()
            .map(|duration| duration.saturating_mul(u64::from(spec.sample_rate)) / 1000);
        let gapless_trim = decoder.gapless_trim_spec().map(|trim| GaplessTrimSpec {
            head_frames: trim.head_frames,
            tail_frames: trim.tail_frames,
        });
        let format = spec;
        self.decoder = Some(decoder);
        Ok(DecodedStreamInfo {
            format,
            duration_frames,
            gapless_trim,
        })
    }

    fn decode(&mut self, output: &mut AudioBlock) -> Result<DecodeStatus, DecodeError> {
        let decoder = self.decoder.as_mut().ok_or_else(|| DecodeError::Failed {
            message: "decoder is not open".to_owned(),
        })?;
        let frames = output.samples.capacity().max(2048)
            / usize::from(output.format.channel_layout.channel_count());
        match decoder.next_block(frames) {
            Ok(Some(samples)) => {
                output.samples = samples;
                Ok(DecodeStatus::Produced {
                    frames: output.frames(),
                })
            },
            Ok(None) => Ok(DecodeStatus::EndOfStream),
            Err(message) if message == SOURCE_PENDING_ERROR => Ok(DecodeStatus::Pending),
            Err(message) if message.starts_with(SOURCE_IO_ERROR_PREFIX) => {
                Err(DecodeError::Io(std::io::Error::other(message)))
            },
            Err(message) => Err(DecodeError::Failed { message }),
        }
    }

    fn start_seek(&mut self, target_frame: u64) -> Result<DecoderSeekStatus, DecodeError> {
        self.pending_seek_frame = Some(target_frame);
        self.continue_seek()
    }

    fn continue_seek(&mut self) -> Result<DecoderSeekStatus, DecodeError> {
        let target_frame = self.pending_seek_frame.ok_or(DecodeError::Unsupported)?;
        let decoder = self.decoder.as_mut().ok_or_else(|| DecodeError::Failed {
            message: "decoder is not open".to_owned(),
        })?;
        let sample_rate = decoder.spec().sample_rate.max(1);
        let millis = target_frame.saturating_mul(1000) / u64::from(sample_rate);
        match decoder.seek_ms(millis) {
            Ok(()) => {
                self.pending_seek_frame = None;
                Ok(DecoderSeekStatus::Complete(SeekResult {
                    actual_frame: target_frame,
                }))
            },
            Err(message) if message == SOURCE_PENDING_ERROR => Ok(DecoderSeekStatus::Pending),
            Err(message) if message.starts_with(SOURCE_IO_ERROR_PREFIX) => {
                self.pending_seek_frame = None;
                Err(DecodeError::Io(std::io::Error::other(message)))
            },
            Err(message) => {
                self.pending_seek_frame = None;
                Err(DecodeError::Failed { message })
            },
        }
    }

    fn reset(&mut self) {}
}

pub struct RuntimeDeviceSinkFactory {
    id: StageId,
    control: DeviceSinkControl,
    route_revision: u64,
}

impl RuntimeDeviceSinkFactory {
    pub fn new(control: DeviceSinkControl, route_revision: u64) -> Self {
        Self {
            id: StageId::new("builtin.sink.device").unwrap(),
            control,
            route_revision,
        }
    }
}

impl SinkFactory for RuntimeDeviceSinkFactory {
    fn id(&self) -> &StageId {
        &self.id
    }

    fn preferred_format(&self, _input: PcmFormat) -> Result<PcmFormat, FactoryError> {
        let (backend, device_id) = self.control.desired_route();
        let spec = crate::device_sink::output_spec_for_route(backend, device_id.as_deref())
            .or_else(|_| crate::device_sink::default_output_spec_for_backend(backend))
            .map_err(|message| FactoryError::InvalidConfiguration { message })?;
        Ok(PcmFormat {
            sample_rate: spec.sample_rate,
            channel_layout: spec.channel_layout,
        })
    }

    fn compatibility_key(&self, format: PcmFormat) -> Result<OutputCompatibilityKey, FactoryError> {
        let (backend, device_id) = self.control.desired_route();
        Ok(OutputCompatibilityKey {
            backend_id: match backend {
                OutputBackend::Shared => "cpal.shared",
                OutputBackend::WasapiExclusive => "wasapi.exclusive",
            }
            .to_owned(),
            device_id,
            sample_rate: format.sample_rate,
            channel_layout: format.channel_layout,
            route_revision: self.route_revision,
        })
    }

    fn create(&self) -> Result<Box<dyn SinkStage>, FactoryError> {
        Ok(Box::new(DeviceSinkStage::with_control(
            self.control.clone(),
        )))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::time::Duration;

    use stellatune_audio_core::{
        error::SourceError,
        source::{
            MediaHints, SourceCancellation, SourceCapabilities, SourceFactory, SourceOpenPurpose,
            SourceOpenRequest,
        },
    };

    use super::HttpSourceFactory;

    #[tokio::test]
    async fn http_source_delivers_headers_and_redacts_debug_output() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut buffer = [0_u8; 1024];
            loop {
                let read = socket.read(&mut buffer).unwrap();
                request.extend_from_slice(&buffer[..read]);
                if read == 0 || request.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            socket
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\ndata",
                )
                .unwrap();
            String::from_utf8(request).unwrap()
        });
        let secret = "Bearer integration-secret";
        let factory = HttpSourceFactory::new(
            format!("http://{address}/stream?credential=hidden"),
            BTreeMap::from([("Authorization".to_owned(), secret.to_owned())]),
            MediaHints::default(),
            SourceCapabilities {
                byte_seekable: true,
                reopenable: true,
                live: false,
            },
        )
        .unwrap();
        let debug = format!("{factory:?}");
        assert!(!debug.contains(secret));
        assert!(!debug.contains("credential=hidden"));
        let mut source = factory
            .open(SourceOpenRequest {
                purpose: SourceOpenPurpose::Initial,
                deadline: None,
                cancellation: SourceCancellation::default(),
            })
            .await
            .unwrap();
        let mut body = Vec::new();
        let mut chunk = [0_u8; 16];
        loop {
            match source.read(&mut chunk) {
                Ok(0) => break,
                Ok(read) => body.extend_from_slice(&chunk[..read]),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::yield_now();
                },
                Err(error) => panic!("HTTP source read failed: {error}"),
            }
        }
        assert_eq!(body, b"data");
        let request = server.join().unwrap();
        assert!(
            request
                .to_ascii_lowercase()
                .contains("authorization: bearer integration-secret")
        );
    }

    #[tokio::test]
    async fn cancelling_slow_http_open_drops_the_request_future_promptly() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut buffer = [0_u8; 1024];
            let _ = socket.read(&mut buffer);
            std::thread::sleep(Duration::from_millis(300));
        });
        let factory = HttpSourceFactory::new(
            format!("http://{address}/slow"),
            BTreeMap::new(),
            MediaHints::default(),
            SourceCapabilities {
                byte_seekable: false,
                reopenable: true,
                live: true,
            },
        )
        .unwrap();
        let cancellation = SourceCancellation::default();
        let canceller = cancellation.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(20)).await;
            canceller.cancel();
        });
        let result = tokio::time::timeout(
            Duration::from_millis(150),
            factory.open(SourceOpenRequest {
                purpose: SourceOpenPurpose::Prewarm,
                deadline: None,
                cancellation,
            }),
        )
        .await
        .expect("cancellation must not wait for the server response");
        assert!(matches!(result, Err(SourceError::Cancelled)));
        server.join().unwrap();
    }
}
