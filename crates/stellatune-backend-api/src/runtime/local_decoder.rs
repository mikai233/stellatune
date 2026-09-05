//! Blocking decoder consumers (metadata probing and transcoding) share the same
//! resolved source as playback, including plugin-hosted HTTP sources.
use std::{
    io::{self, Read, Seek, SeekFrom},
    path::Path,
    time::{Duration, Instant},
};
use stellatune_audio_builtin_adapters::{
    builtin_decoder::BuiltinDecoder, factories::open_builtin_decoder,
};
use stellatune_audio_core::source::{
    EncodedSource, SourceCancellation, SourceOpenPurpose, SourceOpenRequest,
};

pub(crate) struct LocalDecoder {
    pub decoder: BuiltinDecoder,
    pub plugin_id: Option<String>,
    pub capability_id: Option<String>,
}

pub(crate) async fn open_local_decoder(path: &Path) -> Result<LocalDecoder, String> {
    let resolved =
        super::local_source::resolve_local_file(&super::shared_typescript_runtime(), path).await?;
    open_resolved_decoder(resolved).await
}

pub(crate) async fn open_resolved_decoder(
    resolved: super::local_source::ResolvedLocalFile,
) -> Result<LocalDecoder, String> {
    let factory = crate::player_service::resolver::materialize_source(resolved.source)
        .map_err(|error| error.to_string())?;
    let hints = factory.descriptor().media;
    let source = tokio::time::timeout(
        Duration::from_secs(30),
        factory.open(SourceOpenRequest {
            purpose: SourceOpenPurpose::Initial,
            deadline: Some(Instant::now() + Duration::from_secs(30)),
            cancellation: SourceCancellation::default(),
        }),
    )
    .await
    .map_err(|_| "source open timed out".to_owned())?
    .map_err(|error| error.to_string())?;
    tokio::task::spawn_blocking(move || {
        let decoder = open_builtin_decoder(
            Box::new(BlockingSource(source)),
            hints.extension.as_deref().unwrap_or_default(),
        )?;
        Ok(LocalDecoder {
            decoder,
            plugin_id: resolved.plugin_id,
            capability_id: resolved.capability_id,
        })
    })
    .await
    .map_err(|error| error.to_string())?
}

// This adapter is exclusively for blocking workers, never the playback actor.
struct BlockingSource(Box<dyn EncodedSource>);
impl Read for BlockingSource {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            match self.0.read(output) {
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
        self.0.seek(position)
    }
}
impl EncodedSource for BlockingSource {
    fn byte_len(&self) -> Option<u64> {
        self.0.byte_len()
    }
    fn is_seekable(&self) -> bool {
        self.0.is_seekable()
    }
}
