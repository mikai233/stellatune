use std::collections::HashMap;
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use anyhow::Result;
use axum::Router;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode};
use axum::response::IntoResponse;
use axum::routing::get;
use futures_util::StreamExt;
use mime_guess::{MimeGuess, mime};
use stellatune_audio_core::source::SourceCancellation;
use stellatune_backend_api::runtime::segment_wave::SegmentWave;
use tokio::net::TcpListener;
use tokio::sync::{Mutex, RwLock};
use tokio_util::io::ReaderStream;

use super::discovery::candidate_ipv4_addrs;
use super::types::DlnaHttpServerInfo;

#[derive(Clone)]
struct HttpState {
    tracks: Arc<RwLock<HashMap<String, Publication>>>,
}

#[derive(Clone)]
enum Resource {
    File(PathBuf),
    Segment(SegmentWave),
}
#[derive(Clone)]
struct Publication {
    resource: Resource,
    cancellation: SourceCancellation,
    created: std::time::Instant,
}

static HTTP_SERVER: OnceLock<Arc<HttpServer>> = OnceLock::new();
static HTTP_START_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

struct HttpServer {
    info: DlnaHttpServerInfo,
    state: HttpState,
}

pub(super) async fn ensure_http_server(
    advertise_ip: Option<String>,
    port: Option<u16>,
) -> Result<DlnaHttpServerInfo> {
    if let Some(s) = HTTP_SERVER.get() {
        return Ok(s.info.clone());
    }

    let lock = HTTP_START_LOCK.get_or_init(|| Mutex::new(()));
    let _guard = lock.lock().await;
    if let Some(s) = HTTP_SERVER.get() {
        return Ok(s.info.clone());
    }

    let advertise_ip = if let Some(ip) = advertise_ip {
        normalize_advertise_host(&ip)?
    } else {
        default_advertise_host()?
    };

    let bind_port = port.unwrap_or(0);
    let listener = TcpListener::bind(("0.0.0.0", bind_port)).await?;
    let listen_addr = listener.local_addr()?;
    let base_url = format!("http://{}:{}", advertise_ip, listen_addr.port());

    let state = HttpState {
        tracks: Arc::new(RwLock::new(HashMap::new())),
    };

    let app = Router::new()
        .route("/track/{token}", get(http_track).head(http_track))
        .with_state(state.clone());

    tracing::info!(
        "dlna http server starting listen_addr={} base_url={}",
        listen_addr,
        base_url
    );

    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!("dlna http server exited: {e:#}");
        }
    });

    let info = DlnaHttpServerInfo {
        listen_addr: listen_addr.to_string(),
        base_url,
    };
    let server = Arc::new(HttpServer {
        info: info.clone(),
        state,
    });
    let _ = HTTP_SERVER.set(server);

    Ok(info)
}

pub(super) fn default_advertise_host() -> Result<String> {
    // Prefer a private (RFC1918) IPv4 when available (LAN reachable).
    if let Some(ip) = candidate_ipv4_addrs().first().copied() {
        return Ok(ip.to_string());
    }
    Ok(normalize_ipaddr(local_ip_address::local_ip()?))
}

fn normalize_advertise_host(host: &str) -> Result<String> {
    // If it's an IPv6 literal without brackets, add them so `http://HOST:port` is valid.
    let h = host.trim();
    if h.starts_with('[') && h.ends_with(']') {
        return Ok(h.to_string());
    }
    if h.contains(':') {
        // Avoid bracketing if it already looks like "name:port" (single colon only).
        if h.matches(':').count() == 1
            && h.rsplit_once(':')
                .is_some_and(|(_, p)| p.parse::<u16>().is_ok())
        {
            anyhow::bail!("advertise_ip must be a host/ip without port (got {host})");
        }
        return Ok(format!("[{h}]"));
    }
    Ok(h.to_string())
}

fn normalize_ipaddr(ip: IpAddr) -> String {
    match ip {
        IpAddr::V4(v4) => v4.to_string(),
        IpAddr::V6(v6) => format!("[{}]", v6),
    }
}

pub(super) async fn register_track(path: String) -> String {
    register(Resource::File(PathBuf::from(path))).await
}

pub(super) async fn register_segment(wave: SegmentWave) -> String {
    register(Resource::Segment(wave)).await
}

async fn register(resource: Resource) -> String {
    let token = new_token();
    if let Some(server) = HTTP_SERVER.get() {
        let mut tracks = server.state.tracks.write().await;
        while tracks.len() >= 16 {
            let oldest = tracks
                .iter()
                .min_by_key(|(_, p)| p.created)
                .map(|(key, _)| key.clone())
                .unwrap();
            if let Some(old) = tracks.remove(&oldest) {
                old.cancellation.cancel();
            }
        }
        tracks.insert(
            token.clone(),
            Publication {
                resource,
                cancellation: SourceCancellation::default(),
                created: std::time::Instant::now(),
            },
        );
    }
    token
}

fn new_token() -> String {
    use rand::distr::Alphanumeric;
    use rand::{RngExt, rng};
    rng()
        .sample_iter(&Alphanumeric)
        .take(24)
        .map(char::from)
        .collect()
}

async fn http_track(
    State(state): State<HttpState>,
    Path(token): Path<String>,
    headers: HeaderMap,
    method: Method,
) -> impl IntoResponse {
    let range_header = headers
        .get(axum::http::header::RANGE)
        .and_then(|v| v.to_str().ok())
        .filter(|_| method == Method::GET);
    tracing::debug!(
        "dlna http track request method={} token={} range={:?}",
        method,
        token,
        range_header
    );

    let publication = {
        let map = state.tracks.read().await;
        map.get(&token).cloned()
    };
    let Some(publication) = publication else {
        return (StatusCode::NOT_FOUND, "track not found").into_response();
    };

    if let Resource::Segment(wave) = publication.resource {
        return http_segment(wave, publication.cancellation, range_header, method).await;
    }
    let Resource::File(path) = publication.resource else {
        unreachable!()
    };

    let meta = match tokio::fs::metadata(&path).await {
        Ok(m) => m,
        Err(_) => return (StatusCode::NOT_FOUND, "file not found").into_response(),
    };
    let len = meta.len();

    let mut mime = MimeGuess::from_path(&path).first_or_octet_stream();
    if mime.as_ref() == "application/octet-stream"
        && let Ok(Some(detected)) = sniff_mime_from_magic(&path).await
    {
        mime = detected;
    }

    let range = range_header.and_then(|v| parse_single_range(v, len));

    if range_header.is_some() && range.is_none() {
        let mut out_headers = HeaderMap::new();
        let _ = out_headers.insert(
            axum::http::header::CONTENT_RANGE,
            HeaderValue::from_str(&format!("bytes */{}", len))
                .unwrap_or(HeaderValue::from_static("bytes */0")),
        );
        return (StatusCode::RANGE_NOT_SATISFIABLE, out_headers, "").into_response();
    }

    let (status, start, end) = match range {
        Some((s, e)) => (StatusCode::PARTIAL_CONTENT, s, e),
        None => (StatusCode::OK, 0, len.saturating_sub(1)),
    };

    let to_send = if len == 0 {
        0
    } else {
        end.saturating_sub(start) + 1
    };

    let mut out_headers = HeaderMap::new();
    let _ = out_headers.insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_str(mime.as_ref())
            .unwrap_or(HeaderValue::from_static("application/octet-stream")),
    );
    let _ = out_headers.insert(
        axum::http::header::ACCEPT_RANGES,
        HeaderValue::from_static("bytes"),
    );
    let _ = out_headers.insert(
        axum::http::header::CONTENT_LENGTH,
        HeaderValue::from_str(&to_send.to_string()).unwrap_or(HeaderValue::from_static("0")),
    );
    if status == StatusCode::PARTIAL_CONTENT {
        let content_range = format!("bytes {}-{}/{}", start, end, len);
        let _ = out_headers.insert(
            axum::http::header::CONTENT_RANGE,
            HeaderValue::from_str(&content_range).unwrap_or(HeaderValue::from_static("bytes */0")),
        );
    }

    if method == Method::HEAD {
        return (status, out_headers, "").into_response();
    }

    let mut file = match tokio::fs::File::open(&path).await {
        Ok(f) => f,
        Err(_) => return (StatusCode::NOT_FOUND, "file not found").into_response(),
    };

    if start > 0 {
        use tokio::io::AsyncSeekExt;
        if file.seek(std::io::SeekFrom::Start(start)).await.is_err() {
            return (StatusCode::INTERNAL_SERVER_ERROR, "seek failed").into_response();
        }
    }

    use tokio::io::AsyncReadExt;
    let limited = file.take(to_send);
    let cancellation = publication.cancellation;
    let stream =
        ReaderStream::new(limited).take_until(async move { cancellation.cancelled().await });

    (status, out_headers, axum::body::Body::from_stream(stream)).into_response()
}

async fn http_segment(
    wave: SegmentWave,
    cancellation: SourceCancellation,
    range: Option<&str>,
    method: Method,
) -> axum::response::Response {
    let length = wave.length;
    // RFC 9110: Range modifies GET only. Unknown units and unsupported multipart
    // requests are ignored; an unsatisfiable single byte range receives 416.
    let range = range.filter(|value| {
        method == Method::GET
            && value.to_ascii_lowercase().starts_with("bytes=")
            && !value.contains(',')
    });
    let (status, start, end) = if let Some(range) = range {
        let Some((start, end)) = parse_single_range(range, length) else {
            return (
                StatusCode::RANGE_NOT_SATISFIABLE,
                [("content-range", format!("bytes */{length}"))],
                "",
            )
                .into_response();
        };
        (StatusCode::PARTIAL_CONTENT, start, end)
    } else {
        (StatusCode::OK, 0, length - 1)
    };
    let mut headers = HeaderMap::new();
    headers.insert("content-type", HeaderValue::from_static("audio/wav"));
    headers.insert("accept-ranges", HeaderValue::from_static("bytes"));
    headers.insert(
        "content-length",
        HeaderValue::from_str(&(end - start + 1).to_string()).unwrap(),
    );
    if status == StatusCode::PARTIAL_CONTENT {
        headers.insert(
            "content-range",
            HeaderValue::from_str(&format!("bytes {start}-{end}/{length}")).unwrap(),
        );
    }
    if method == Method::HEAD {
        return (status, headers, "").into_response();
    }
    let receiver = match wave.stream(start, end, cancellation.clone()).await {
        Ok(receiver) => receiver,
        Err(error) => return (StatusCode::SERVICE_UNAVAILABLE, error.to_string()).into_response(),
    };
    let stream = futures_util::stream::unfold(
        (receiver, cancellation),
        |(mut receiver, cancellation)| async move {
            tokio::select! { biased;
                _ = cancellation.cancelled() => None,
                value = receiver.recv() => value.map(|value| (value, (receiver, cancellation))),
            }
        },
    );
    (status, headers, axum::body::Body::from_stream(stream)).into_response()
}

fn parse_single_range(header: &str, len: u64) -> Option<(u64, u64)> {
    // Only supports a single range of the form:
    // - bytes=start-end
    // - bytes=start-
    // - bytes=-suffix
    let header = header.trim();
    let lower = header.to_ascii_lowercase();
    let rest = lower.strip_prefix("bytes=")?;
    if rest.contains(',') {
        return None;
    }
    let (a, b) = rest.split_once('-')?;
    if len == 0 {
        return None;
    }

    let last = len - 1;

    if a.is_empty() {
        // suffix range: "-N"
        let suffix: u64 = b.parse().ok()?;
        if suffix == 0 {
            return None;
        }
        let start = len.saturating_sub(suffix);
        return Some((start, last));
    }

    let start: u64 = a.parse().ok()?;
    if start >= len {
        return None;
    }

    if b.is_empty() {
        return Some((start, last));
    }

    let mut end: u64 = b.parse().ok()?;
    if end >= len {
        end = last;
    }
    if end < start {
        return None;
    }
    Some((start, end))
}

async fn sniff_mime_from_magic(path: &PathBuf) -> Result<Option<mime::Mime>> {
    use tokio::io::AsyncReadExt;
    let mut f = tokio::fs::File::open(path).await?;
    let mut buf = [0u8; 16];
    let n = f.read(&mut buf).await?;
    let b = &buf[..n];

    // JPEG
    if b.len() >= 3 && b[0] == 0xFF && b[1] == 0xD8 && b[2] == 0xFF {
        return Ok(Some("image/jpeg".parse().unwrap()));
    }
    // PNG
    if b.len() >= 8
        && b[0] == 0x89
        && b[1] == 0x50
        && b[2] == 0x4E
        && b[3] == 0x47
        && b[4] == 0x0D
        && b[5] == 0x0A
        && b[6] == 0x1A
        && b[7] == 0x0A
    {
        return Ok(Some("image/png".parse().unwrap()));
    }
    // GIF
    if b.len() >= 6 && (&b[..6] == b"GIF87a" || &b[..6] == b"GIF89a") {
        return Ok(Some("image/gif".parse().unwrap()));
    }

    Ok(None)
}

pub(super) async fn unpublish_all() {
    if let Some(server) = HTTP_SERVER.get() {
        for (_, publication) in server.state.tracks.write().await.drain() {
            publication.cancellation.cancel();
        }
    }
}

pub(super) async fn retain_urls(urls: &[Option<&str>]) {
    if let Some(server) = HTTP_SERVER.get() {
        let tokens = urls
            .iter()
            .flatten()
            .filter_map(|url| url.rsplit('/').next())
            .collect::<Vec<_>>();
        server
            .state
            .tracks
            .write()
            .await
            .retain(|token, publication| {
                if tokens.contains(&token.as_str()) {
                    true
                } else {
                    publication.cancellation.cancel();
                    false
                }
            });
    }
}

pub(super) async fn remove_urls(urls: &[Option<&str>]) {
    if let Some(server) = HTTP_SERVER.get() {
        let mut tracks = server.state.tracks.write().await;
        for token in urls
            .iter()
            .flatten()
            .filter_map(|url| url.rsplit('/').next())
        {
            if let Some(publication) = tracks.remove(token) {
                publication.cancellation.cancel();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stellatune_audio_core::segment::AudioSegment;
    use stellatune_library::catalog::LocalTrackResource;

    #[tokio::test]
    async fn segment_http_head_ranges_and_publication_cleanup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audio.wav");
        let mut wav = b"RIFF".to_vec();
        wav.extend(2036_u32.to_le_bytes());
        wav.extend(b"WAVEfmt ");
        wav.extend(16_u32.to_le_bytes());
        wav.extend(1_u16.to_le_bytes());
        wav.extend(1_u16.to_le_bytes());
        wav.extend(48000_u32.to_le_bytes());
        wav.extend(96000_u32.to_le_bytes());
        wav.extend(2_u16.to_le_bytes());
        wav.extend(16_u16.to_le_bytes());
        wav.extend(b"data");
        wav.extend(2000_u32.to_le_bytes());
        for i in 0..1000_i16 {
            wav.extend(i.to_le_bytes());
        }
        std::fs::write(&path, wav).unwrap();
        let wave = SegmentWave::prepare(LocalTrackResource {
            path: path.to_string_lossy().into_owned(),
            segment: Some(AudioSegment {
                start_frame: 37,
                end_frame_exclusive: 891,
                sample_rate: 48000,
            }),
            cover_key: 1,
            pcm_bits: Some(16),
            pcm_float: false,
        })
        .await
        .unwrap();
        let length = wave.length;
        let info = ensure_http_server(Some("127.0.0.1".into()), Some(0))
            .await
            .unwrap();
        let address: std::net::SocketAddr = info.listen_addr.parse().unwrap();
        let token = register_segment(wave).await;
        let url = format!("http://127.0.0.1:{}/track/{token}", address.port());
        let client = reqwest::Client::new();
        let head = client
            .head(&url)
            .header("Range", "bytes=1-2")
            .send()
            .await
            .unwrap();
        assert_eq!(head.status(), StatusCode::OK);
        assert_eq!(head.headers()["content-length"], length.to_string());
        assert!(head.bytes().await.unwrap().is_empty());
        let full = client
            .get(&url)
            .send()
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap();
        assert_eq!(full.len() as u64, length);
        for range in ["bytes=3-49", "bytes=-7", "bytes=77-"] {
            let (start, end) = parse_single_range(range, length).unwrap();
            let response = client
                .get(&url)
                .header("Range", range)
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
            assert_eq!(
                response.headers()["content-range"],
                format!("bytes {start}-{end}/{length}")
            );
            assert_eq!(
                response.bytes().await.unwrap(),
                &full[start as usize..=end as usize]
            );
        }
        let invalid = client
            .get(&url)
            .header("Range", format!("bytes={length}-"))
            .send()
            .await
            .unwrap();
        assert_eq!(invalid.status(), StatusCode::RANGE_NOT_SATISFIABLE);
        assert_eq!(
            invalid.headers()["content-range"],
            format!("bytes */{length}")
        );
        let server = HTTP_SERVER.get().unwrap();
        let cancellation = server.state.tracks.read().await[&token]
            .cancellation
            .clone();
        for _ in 0..20 {
            register_track(path.to_string_lossy().into_owned()).await;
        }
        assert_eq!(server.state.tracks.read().await.len(), 16);
        assert!(cancellation.is_cancelled());
        unpublish_all().await;
        assert!(server.state.tracks.read().await.is_empty());
        assert_eq!(
            client.get(&url).send().await.unwrap().status(),
            StatusCode::NOT_FOUND
        );
    }
}
