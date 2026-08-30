use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use stellatune_audio::pipeline::capability::{
    ResolveSourceRequest, SourceResolveError, SourceResolver, SourceResolverFactory,
};
use stellatune_audio::pipeline::plan::{
    MediaHints, SourceCapabilities, SourceLocator, SourcePlan, SourceRequirements, StageConfig,
    StageId,
};
use stellatune_plugins::typescript::TypeScriptRuntime;
use stellatune_plugins::typescript::protocol::{SourceLocatorDto, SourcePlanDto};

pub struct TypeScriptSourceResolverProxy {
    runtime: Arc<TypeScriptRuntime>,
    plugin_id: String,
    capability_id: String,
}

impl TypeScriptSourceResolverProxy {
    pub fn new(
        runtime: Arc<TypeScriptRuntime>,
        plugin_id: impl Into<String>,
        capability_id: impl Into<String>,
    ) -> Self {
        Self {
            runtime,
            plugin_id: plugin_id.into(),
            capability_id: capability_id.into(),
        }
    }
}

impl SourceResolver for TypeScriptSourceResolverProxy {
    fn resolve<'a>(
        &'a self,
        request: &'a ResolveSourceRequest,
    ) -> Pin<Box<dyn Future<Output = Result<SourcePlan, SourceResolveError>> + Send + 'a>> {
        Box::pin(async move {
            let mut input = request.input.clone();
            if let Some(object) = input.as_object_mut() {
                object.insert("config".to_string(), request.config.value().clone());
            }
            let invocation = self
                .runtime
                .invoke(
                    &self.plugin_id,
                    &self.capability_id,
                    None,
                    "resolve",
                    input,
                    None,
                )
                .await
                .map_err(|error| self.error(error.to_string()))?;
            let dto: SourcePlanDto = serde_json::from_value(invocation.value)
                .map_err(|error| self.error(format!("invalid SourcePlan DTO: {error}")))?;
            map_source_plan(dto).map_err(|message| self.error(message))
        })
    }
}

impl TypeScriptSourceResolverProxy {
    fn error(&self, message: String) -> SourceResolveError {
        SourceResolveError {
            capability_id: format!("{}::{}", self.plugin_id, self.capability_id),
            message,
        }
    }
}

pub struct TypeScriptSourceResolverFactory {
    runtime: Arc<TypeScriptRuntime>,
    plugin_id: String,
    capability_id: String,
}

impl TypeScriptSourceResolverFactory {
    pub fn new(
        runtime: Arc<TypeScriptRuntime>,
        plugin_id: impl Into<String>,
        capability_id: impl Into<String>,
    ) -> Self {
        Self {
            runtime,
            plugin_id: plugin_id.into(),
            capability_id: capability_id.into(),
        }
    }
}

impl SourceResolverFactory for TypeScriptSourceResolverFactory {
    fn create(&self, _config: &StageConfig) -> Result<Arc<dyn SourceResolver>, SourceResolveError> {
        Ok(Arc::new(TypeScriptSourceResolverProxy::new(
            Arc::clone(&self.runtime),
            self.plugin_id.clone(),
            self.capability_id.clone(),
        )))
    }
}

fn map_source_plan(dto: SourcePlanDto) -> Result<SourcePlan, String> {
    let (locator, extension) = match dto.source {
        SourceLocatorDto::File { path } => {
            let extension = extension_hint(&path);
            (SourceLocator::File { path }, extension)
        },
        SourceLocatorDto::Http { url, headers } => {
            if !(url.starts_with("http://") || url.starts_with("https://")) {
                return Err("HTTP SourcePlan URL must use http or https".to_string());
            }
            let extension = extension_hint(&url);
            (SourceLocator::Http { url, headers }, extension)
        },
    };
    let extension = dto
        .media
        .codec_hint
        .filter(|value| !value.trim().is_empty())
        .or(extension);
    let decoder = dto
        .requirements
        .decoder_capability_id
        .map(StageId::new)
        .transpose()
        .map_err(str::to_string)?;
    Ok(SourcePlan {
        locator,
        media: MediaHints {
            extension,
            mime_type: dto.media.mime_type,
            content_length: None,
        },
        capabilities: SourceCapabilities {
            seekable: dto.capabilities.seekable,
            live: false,
        },
        requirements: SourceRequirements { decoder },
    })
}

fn extension_hint(locator: &str) -> Option<String> {
    let clean = locator.split(['?', '#']).next().unwrap_or(locator);
    std::path::Path::new(clean)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .filter(|extension| !extension.is_empty())
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::thread;

    use serde_json::json;
    use stellatune_audio::pipeline::capability::{ResolveSourceRequest, SourceResolver};
    use stellatune_audio::pipeline::plan::{SourceLocator, StageConfig};
    use stellatune_audio_builtin_adapters::builtin_decoder::BuiltinDecoder;
    use stellatune_plugins::typescript::TypeScriptRuntime;
    use stellatune_plugins::typescript::manifest::read_typescript_manifest;

    use super::TypeScriptSourceResolverProxy;

    fn repository_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("backend crate must be under repository/crates")
            .to_path_buf()
    }

    #[tokio::test]
    async fn resolver_returns_http_plan_and_rust_fetches_bytes_after_node_stops() {
        let root = repository_root();
        let fixture_root = root.join("tools/typescript-plugin-runtime/fixtures");
        let manifest = read_typescript_manifest(&fixture_root.join("manifest.json")).unwrap();
        let runtime = Arc::new(TypeScriptRuntime::new(
            root.join("tools/typescript-plugin-runtime/runner.mjs"),
        ));
        runtime.register(manifest, &fixture_root).await.unwrap();

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request);
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 12\r\n\r\nencoded-data")
                .unwrap();
        });
        let media_url = format!("http://{address}/fixture.flac");
        let resolver = TypeScriptSourceResolverProxy::new(
            Arc::clone(&runtime),
            "dev.stellatune.fixture.http-source",
            "fixture-source",
        );
        let plan = resolver
            .resolve(&ResolveSourceRequest {
                input: json!({ "url": media_url }),
                config: StageConfig::default(),
            })
            .await
            .unwrap();
        let SourceLocator::Http { url, headers } = plan.locator else {
            panic!("fixture must return HTTP SourcePlan");
        };
        assert_eq!(headers.get("x-stellatune-fixture").unwrap(), "1");

        runtime
            .stop_process("dev.stellatune.fixture.http-source")
            .await
            .unwrap();
        let response = reqwest::get(url).await.unwrap();
        assert_eq!(response.bytes().await.unwrap().as_ref(), b"encoded-data");
        server.join().unwrap();
        runtime.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn first_party_netease_resolves_then_native_rust_decodes_http_wav() {
        let root = repository_root();
        let source = root.join("crates/plugins-native/stellatune-plugin-netease");
        let package = tempfile::tempdir().unwrap();
        for file in ["manifest.json", "plugin.mjs", "source-config.schema.json"] {
            std::fs::copy(source.join(file), package.path().join(file)).unwrap();
        }
        std::fs::create_dir(package.path().join("ui")).unwrap();
        std::fs::write(package.path().join("ui/index.html"), "<!doctype html>").unwrap();

        let wav = tiny_wav_bytes();
        let media = TcpListener::bind("127.0.0.1:0").unwrap();
        let media_address = media.local_addr().unwrap();
        let media_server = thread::spawn(move || serve_wav(media, wav, 2));

        let sidecar = TcpListener::bind("127.0.0.1:0").unwrap();
        let sidecar_address = sidecar.local_addr().unwrap();
        let media_url = format!("http://{media_address}/native.wav");
        let sidecar_server = thread::spawn(move || {
            for index in 0..2 {
                let (mut stream, _) = sidecar.accept().unwrap();
                let mut request = [0_u8; 2048];
                let read = stream.read(&mut request).unwrap();
                let request = String::from_utf8_lossy(&request[..read]);
                let body = if index == 0 {
                    assert!(request.starts_with("GET /health"));
                    r#"{"ok":true}"#.to_string()
                } else {
                    assert!(request.starts_with("GET /v1/song/url?"));
                    serde_json::json!({ "url": media_url, "ext_hint": "wav" }).to_string()
                };
                write_http_response(&mut stream, "application/json", body.as_bytes(), None);
            }
        });

        let manifest = read_typescript_manifest(&package.path().join("manifest.json")).unwrap();
        let runtime = Arc::new(TypeScriptRuntime::new(
            root.join("tools/typescript-plugin-runtime/runner.mjs"),
        ));
        runtime.register(manifest, package.path()).await.unwrap();
        let resolver = TypeScriptSourceResolverProxy::new(
            Arc::clone(&runtime),
            "dev.stellatune.source.netease",
            "netease-source",
        );
        let plan = resolver
            .resolve(&ResolveSourceRequest {
                input: json!({ "song_id": 42 }),
                config: StageConfig::validated(json!({
                    "sidecarBaseUrl": format!("http://{sidecar_address}")
                })),
            })
            .await
            .unwrap();
        let SourceLocator::Http { url, .. } = plan.locator else {
            panic!("Netease resolver must return HTTP");
        };
        runtime.shutdown().await.unwrap();
        let decoder = tokio::task::spawn_blocking(move || {
            let mut decoder = BuiltinDecoder::open(&url).unwrap();
            let samples = decoder.next_block(64).unwrap().unwrap();
            (decoder.spec(), samples)
        })
        .await
        .unwrap();
        assert_eq!(decoder.0.sample_rate, 44_100);
        assert_eq!(decoder.0.channels, 1);
        assert!(!decoder.1.is_empty());
        sidecar_server.join().unwrap();
        media_server.join().unwrap();
    }

    fn serve_wav(listener: TcpListener, body: Vec<u8>, requests: usize) {
        for _ in 0..requests {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 4096];
            let read = stream.read(&mut request).unwrap();
            let request = String::from_utf8_lossy(&request[..read]);
            let range = request
                .lines()
                .find(|line| line.to_ascii_lowercase().starts_with("range:"));
            if let Some(range) = range {
                let value = range.split_once(':').unwrap().1.trim();
                let value = value.strip_prefix("bytes=").unwrap();
                let (start, end) = value.split_once('-').unwrap();
                let start = start.parse::<usize>().unwrap().min(body.len() - 1);
                let end = if end.is_empty() {
                    body.len() - 1
                } else {
                    end.parse::<usize>().unwrap().min(body.len() - 1)
                };
                write_http_response(
                    &mut stream,
                    "audio/wav",
                    &body[start..=end],
                    Some(format!("bytes {start}-{end}/{}", body.len())),
                );
            } else {
                write_http_response(&mut stream, "audio/wav", &body, None);
            }
        }
    }

    fn write_http_response(
        stream: &mut std::net::TcpStream,
        content_type: &str,
        body: &[u8],
        content_range: Option<String>,
    ) {
        let status = if content_range.is_some() {
            "206 Partial Content"
        } else {
            "200 OK"
        };
        let range_header = content_range
            .map(|value| format!("Content-Range: {value}\r\n"))
            .unwrap_or_default();
        write!(
            stream,
            "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nAccept-Ranges: bytes\r\nContent-Length: {}\r\n{range_header}Connection: close\r\n\r\n",
            body.len()
        )
        .unwrap();
        stream.write_all(body).unwrap();
    }

    fn tiny_wav_bytes() -> Vec<u8> {
        let samples = [0_i16, 1200, -1200, 0].repeat(11_025);
        let data_size = (samples.len() * 2) as u32;
        let mut out = Vec::with_capacity(44 + data_size as usize);
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data_size).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16_u32.to_le_bytes());
        out.extend_from_slice(&1_u16.to_le_bytes());
        out.extend_from_slice(&1_u16.to_le_bytes());
        out.extend_from_slice(&44_100_u32.to_le_bytes());
        out.extend_from_slice(&88_200_u32.to_le_bytes());
        out.extend_from_slice(&2_u16.to_le_bytes());
        out.extend_from_slice(&16_u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&data_size.to_le_bytes());
        for sample in samples {
            out.extend_from_slice(&sample.to_le_bytes());
        }
        out
    }
}
