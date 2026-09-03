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
use mime_guess::{MimeGuess, mime};
use tokio::net::TcpListener;
use tokio::sync::{Mutex, RwLock};
use tokio_util::io::ReaderStream;

use super::discovery::candidate_ipv4_addrs;
use super::types::DlnaHttpServerInfo;

#[derive(Clone)]
struct HttpState {
    tracks: Arc<RwLock<HashMap<String, PathBuf>>>,
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
    let token = new_token();
    if let Some(server) = HTTP_SERVER.get() {
        server
            .state
            .tracks
            .write()
            .await
            .insert(token.clone(), PathBuf::from(path));
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
        .and_then(|v| v.to_str().ok());
    tracing::debug!(
        "dlna http track request method={} token={} range={:?}",
        method,
        token,
        range_header
    );

    let path = {
        let map = state.tracks.read().await;
        map.get(&token).cloned()
    };
    let Some(path) = path else {
        return (StatusCode::NOT_FOUND, "track not found").into_response();
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
    let stream = ReaderStream::new(limited);

    (status, out_headers, axum::body::Body::from_stream(stream)).into_response()
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
        server.state.tracks.write().await.clear();
    }
}
