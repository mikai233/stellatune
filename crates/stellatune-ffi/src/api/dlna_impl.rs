use std::collections::HashMap;
use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Duration;

use anyhow::Result;
use axum::Router;
use axum::extract::{Path, State};
use axum::http::Method;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::routing::get;
use mime_guess::MimeGuess;
use mime_guess::mime;
use roxmltree::Document;
use socket2::{Domain, Protocol, Socket, Type};
use stellatune_core::{
    DlnaHttpServerInfo, DlnaPositionInfo, DlnaRenderer, DlnaSsdpDevice, DlnaTransportInfo,
};
use tokio::net::TcpListener;
use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use tokio::sync::RwLock;
use tokio::task::JoinSet;
use tokio::time::Instant;
use tokio_util::io::ReaderStream;
use url::Url;

const SSDP_ADDR_V4: &str = "239.255.255.250:1900";
const ST_MEDIA_RENDERER: &str = "urn:schemas-upnp-org:device:MediaRenderer:1";
const ST_AV_TRANSPORT: &str = "urn:schemas-upnp-org:service:AVTransport:";
const ST_RENDERING_CONTROL: &str = "urn:schemas-upnp-org:service:RenderingControl:";
const SERVICE_AV_TRANSPORT_1: &str = "urn:schemas-upnp-org:service:AVTransport:1";
const SERVICE_RENDERING_CONTROL_1: &str = "urn:schemas-upnp-org:service:RenderingControl:1";

pub(crate) struct Dlna {}

impl Dlna {
    pub(crate) async fn discover_media_renderers(timeout_ms: u32) -> Result<Vec<DlnaSsdpDevice>> {
        let timeout = Duration::from_millis(timeout_ms.max(200) as u64);
        ssdp_msearch_multi_iface(ST_MEDIA_RENDERER, 1, timeout).await
    }

    pub(crate) async fn discover_renderers(timeout_ms: u32) -> Result<Vec<DlnaRenderer>> {
        let devices = Self::discover_media_renderers(timeout_ms).await?;
        if devices.is_empty() {
            return Ok(Vec::new());
        }

        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(timeout_ms.max(2000) as u64))
            .build()?;

        let mut join = JoinSet::new();
        for d in devices {
            let client = client.clone();
            join.spawn(async move { describe_renderer(&client, d).await });
        }

        let mut out = Vec::new();
        while let Some(res) = join.join_next().await {
            match res {
                Ok(Ok(Some(renderer))) => out.push(renderer),
                Ok(Ok(None)) => {}
                Ok(Err(e)) => tracing::debug!("dlna describe failed: {e:#}"),
                Err(e) => tracing::debug!("dlna describe task join error: {e}"),
            }
        }

        out.sort_by(|a, b| {
            a.friendly_name
                .to_lowercase()
                .cmp(&b.friendly_name.to_lowercase())
        });
        Ok(out)
    }

    pub(crate) async fn http_start(
        advertise_ip: Option<String>,
        port: Option<u16>,
    ) -> Result<DlnaHttpServerInfo> {
        ensure_http_server(advertise_ip, port).await
    }

    pub(crate) async fn http_publish_track(path: String) -> Result<String> {
        let info = ensure_http_server(None, None).await?;
        let token = register_track(path).await;
        // Even if the HTTP server is already running, compute the advertised host at publish-time.
        // This prevents a VPN/tunnel default route change from making the previously chosen
        // `base_url` unreachable from the DLNA renderer.
        let listen_addr: SocketAddr = info.listen_addr.parse()?;
        let host = default_advertise_host()?;
        let url = format!("http://{}:{}/track/{}", host, listen_addr.port(), token);
        tracing::info!("dlna publish track url={}", url);
        Ok(url)
    }

    pub(crate) async fn http_unpublish_all() -> Result<()> {
        if let Some(server) = HTTP_SERVER.get() {
            server.state.tracks.write().await.clear();
        }
        Ok(())
    }

    pub(crate) async fn av_transport_set_uri(
        control_url: String,
        service_type: Option<String>,
        uri: String,
        metadata: Option<String>,
    ) -> Result<()> {
        let client = reqwest::Client::new();
        let service = service_type.as_deref().unwrap_or(SERVICE_AV_TRANSPORT_1);
        let meta = metadata.unwrap_or_default();
        let body = format!(
            "<InstanceID>0</InstanceID>\
<CurrentURI>{}</CurrentURI>\
<CurrentURIMetaData>{}</CurrentURIMetaData>",
            escape_xml(&uri),
            escape_xml(&meta)
        );
        soap_call(&client, &control_url, service, "SetAVTransportURI", &body).await?;
        Ok(())
    }

    pub(crate) async fn av_transport_play(
        control_url: String,
        service_type: Option<String>,
    ) -> Result<()> {
        let client = reqwest::Client::new();
        let service = service_type.as_deref().unwrap_or(SERVICE_AV_TRANSPORT_1);
        soap_call(
            &client,
            &control_url,
            service,
            "Play",
            "<InstanceID>0</InstanceID><Speed>1</Speed>",
        )
        .await?;
        Ok(())
    }

    pub(crate) async fn av_transport_pause(
        control_url: String,
        service_type: Option<String>,
    ) -> Result<()> {
        let client = reqwest::Client::new();
        let service = service_type.as_deref().unwrap_or(SERVICE_AV_TRANSPORT_1);
        soap_call(
            &client,
            &control_url,
            service,
            "Pause",
            "<InstanceID>0</InstanceID>",
        )
        .await?;
        Ok(())
    }

    pub(crate) async fn av_transport_stop(
        control_url: String,
        service_type: Option<String>,
    ) -> Result<()> {
        let client = reqwest::Client::new();
        let service = service_type.as_deref().unwrap_or(SERVICE_AV_TRANSPORT_1);
        soap_call(
            &client,
            &control_url,
            service,
            "Stop",
            "<InstanceID>0</InstanceID>",
        )
        .await?;
        Ok(())
    }

    pub(crate) async fn av_transport_seek_ms(
        control_url: String,
        service_type: Option<String>,
        position_ms: u64,
    ) -> Result<()> {
        let client = reqwest::Client::new();
        let service = service_type.as_deref().unwrap_or(SERVICE_AV_TRANSPORT_1);
        let target = ms_to_hhmmss(position_ms);
        let body = format!(
            "<InstanceID>0</InstanceID>\
<Unit>REL_TIME</Unit>\
<Target>{}</Target>",
            escape_xml(&target)
        );
        soap_call(&client, &control_url, service, "Seek", &body).await?;
        Ok(())
    }

    pub(crate) async fn av_transport_get_transport_info(
        control_url: String,
        service_type: Option<String>,
    ) -> Result<DlnaTransportInfo> {
        let client = reqwest::Client::new();
        let service = service_type.as_deref().unwrap_or(SERVICE_AV_TRANSPORT_1);
        let xml = soap_call(
            &client,
            &control_url,
            service,
            "GetTransportInfo",
            "<InstanceID>0</InstanceID>",
        )
        .await?;

        let state =
            soap_get_text(&xml, "CurrentTransportState").unwrap_or_else(|| "UNKNOWN".to_string());
        let status = soap_get_text(&xml, "CurrentTransportStatus");
        let speed = soap_get_text(&xml, "CurrentSpeed");
        Ok(DlnaTransportInfo {
            current_transport_state: state,
            current_transport_status: status,
            current_speed: speed,
        })
    }

    pub(crate) async fn av_transport_get_position_info(
        control_url: String,
        service_type: Option<String>,
    ) -> Result<DlnaPositionInfo> {
        let client = reqwest::Client::new();
        let service = service_type.as_deref().unwrap_or(SERVICE_AV_TRANSPORT_1);
        let xml = soap_call(
            &client,
            &control_url,
            service,
            "GetPositionInfo",
            "<InstanceID>0</InstanceID>",
        )
        .await?;

        let rel_time_ms = soap_get_text(&xml, "RelTime")
            .as_deref()
            .and_then(hhmmss_to_ms)
            .unwrap_or(0);
        let track_duration_ms = soap_get_text(&xml, "TrackDuration")
            .as_deref()
            .and_then(hhmmss_to_ms);
        Ok(DlnaPositionInfo {
            rel_time_ms,
            track_duration_ms,
        })
    }

    pub(crate) async fn rendering_control_set_volume(
        control_url: String,
        service_type: Option<String>,
        volume_0_100: u8,
    ) -> Result<()> {
        let client = reqwest::Client::new();
        let service = service_type
            .as_deref()
            .unwrap_or(SERVICE_RENDERING_CONTROL_1);
        rendering_control_try_channels_action(
            &client,
            &control_url,
            service,
            "SetVolume",
            |channel| {
                format!(
                    "<InstanceID>0</InstanceID>\
<Channel>{}</Channel>\
<DesiredVolume>{}</DesiredVolume>",
                    escape_xml(channel),
                    volume_0_100
                )
            },
        )
        .await?;
        Ok(())
    }

    pub(crate) async fn rendering_control_set_mute(
        control_url: String,
        service_type: Option<String>,
        mute: bool,
    ) -> Result<()> {
        let client = reqwest::Client::new();
        let service = service_type
            .as_deref()
            .unwrap_or(SERVICE_RENDERING_CONTROL_1);
        let desired = if mute { 1 } else { 0 };
        rendering_control_try_channels_action(
            &client,
            &control_url,
            service,
            "SetMute",
            |channel| {
                format!(
                    "<InstanceID>0</InstanceID>\
<Channel>{}</Channel>\
<DesiredMute>{}</DesiredMute>",
                    escape_xml(channel),
                    desired
                )
            },
        )
        .await?;
        Ok(())
    }

    pub(crate) async fn rendering_control_get_volume(
        control_url: String,
        service_type: Option<String>,
    ) -> Result<u8> {
        let client = reqwest::Client::new();
        let service = service_type
            .as_deref()
            .unwrap_or(SERVICE_RENDERING_CONTROL_1);

        for channel in ["Master", "LF", "RF"] {
            let body = format!(
                "<InstanceID>0</InstanceID><Channel>{}</Channel>",
                escape_xml(channel)
            );
            let resp = soap_call(&client, &control_url, service, "GetVolume", &body).await;
            let Ok(resp) = resp else { continue };
            if let Some(v) = parse_u8_from_soap(&resp, "CurrentVolume") {
                return Ok(v);
            }
        }

        anyhow::bail!("failed to parse CurrentVolume from SOAP response")
    }

    pub(crate) async fn play_local_path(renderer: DlnaRenderer, path: String) -> Result<String> {
        Self::play_local_track(renderer, path, None, None, None, None).await
    }

    pub(crate) async fn play_local_track(
        renderer: DlnaRenderer,
        path: String,
        title: Option<String>,
        artist: Option<String>,
        album: Option<String>,
        cover_path: Option<String>,
    ) -> Result<String> {
        let Some(control_url) = renderer.av_transport_control_url.clone() else {
            anyhow::bail!("renderer has no AVTransport control URL");
        };
        let service_type = renderer.av_transport_service_type.clone();

        let url = Self::http_publish_track(path.clone()).await?;

        let cover_url = if let Some(cp) = cover_path {
            match tokio::fs::metadata(&cp).await {
                Ok(_) => Some(Self::http_publish_track(cp).await?),
                Err(_) => None,
            }
        } else {
            None
        };

        let meta = build_didl_metadata(&url, &path, title, artist, album, cover_url.as_deref());

        Self::av_transport_set_uri(
            control_url.clone(),
            service_type.clone(),
            url.clone(),
            Some(meta),
        )
        .await?;
        Self::av_transport_play(control_url, service_type).await?;
        Ok(url)
    }
}

fn build_didl_metadata(
    track_url: &str,
    track_path: &str,
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    cover_url: Option<&str>,
) -> String {
    let fallback_title = track_path
        .rsplit(|c| c == '/' || c == '\\')
        .next()
        .unwrap_or(track_path);
    let title = title
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(fallback_title);

    let mime = MimeGuess::from_path(track_path).first_or_octet_stream();
    let protocol_info = format!("http-get:*:{}:*", mime.as_ref());

    let mut didl = String::new();
    didl.push_str(
        r#"<DIDL-Lite xmlns="urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:upnp="urn:schemas-upnp-org:metadata-1-0/upnp/" xmlns:dlna="urn:schemas-dlna-org:metadata-1-0/">"#,
    );
    didl.push_str(r#"<item id="0" parentID="0" restricted="1">"#);
    didl.push_str(&format!("<dc:title>{}</dc:title>", escape_xml(title)));
    if let Some(a) = artist.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        didl.push_str(&format!("<dc:creator>{}</dc:creator>", escape_xml(a)));
        didl.push_str(&format!("<upnp:artist>{}</upnp:artist>", escape_xml(a)));
    }
    if let Some(a) = album.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        didl.push_str(&format!("<upnp:album>{}</upnp:album>", escape_xml(a)));
    }
    didl.push_str("<upnp:class>object.item.audioItem.musicTrack</upnp:class>");
    if let Some(c) = cover_url {
        didl.push_str(&format!(
            "<upnp:albumArtURI>{}</upnp:albumArtURI>",
            escape_xml(c)
        ));
    }
    didl.push_str(&format!(
        "<res protocolInfo=\"{}\">{}</res>",
        escape_xml(&protocol_info),
        escape_xml(track_url)
    ));
    didl.push_str("</item></DIDL-Lite>");
    didl
}

async fn ssdp_msearch_multi_iface(
    st: &str,
    mx: u8,
    timeout: Duration,
) -> Result<Vec<DlnaSsdpDevice>> {
    let ips = candidate_ipv4_addrs();

    // Fallback: bind to 0.0.0.0 if we can't enumerate interfaces for some reason.
    if ips.is_empty() {
        return ssdp_msearch_on_socket(
            UdpSocket::bind(("0.0.0.0", 0)).await?,
            None,
            st.to_string(),
            mx,
            timeout,
        )
        .await;
    }

    let mut join = JoinSet::new();
    for ip in ips {
        let st = st.to_string();
        join.spawn(async move {
            let socket = bind_udp_on_iface(ip)?;
            ssdp_msearch_on_socket(socket, Some(ip), st, mx, timeout).await
        });
    }

    let mut devices = Vec::new();
    let mut seen_usn: HashSet<String> = HashSet::new();
    while let Some(res) = join.join_next().await {
        match res {
            Ok(Ok(list)) => {
                for d in list {
                    if seen_usn.insert(d.usn.clone()) {
                        devices.push(d);
                    }
                }
            }
            Ok(Err(e)) => tracing::debug!("ssdp m-search iface task failed: {e:#}"),
            Err(e) => tracing::debug!("ssdp m-search iface join failed: {e}"),
        }
    }

    Ok(devices)
}

async fn ssdp_msearch_on_socket(
    socket: UdpSocket,
    local_ip: Option<Ipv4Addr>,
    st: String,
    mx: u8,
    timeout: Duration,
) -> Result<Vec<DlnaSsdpDevice>> {
    socket.set_broadcast(true)?;
    // Some platforms return an error for multicast TTL; it's fine to ignore.
    let _ = socket.set_multicast_ttl_v4(2);

    let req = format!(
        "M-SEARCH * HTTP/1.1\r\n\
HOST: {SSDP_ADDR_V4}\r\n\
MAN: \"ssdp:discover\"\r\n\
MX: {mx}\r\n\
ST: {st}\r\n\
\r\n"
    );
    tracing::debug!(
        "ssdp m-search st={st} mx={mx} timeout_ms={} local_ip={:?}",
        timeout.as_millis(),
        local_ip
    );
    socket.send_to(req.as_bytes(), SSDP_ADDR_V4).await?;

    let deadline = Instant::now() + timeout;
    let mut buf = [0u8; 8192];
    let mut devices = Vec::new();
    let mut seen_usn: HashSet<String> = HashSet::new();

    loop {
        let now = Instant::now();
        if now >= deadline {
            break;
        }
        let remaining = deadline - now;
        let remaining = remaining.min(Duration::from_millis(250));

        let recv = tokio::time::timeout(remaining, socket.recv_from(&mut buf)).await;
        match recv {
            Ok(Ok((len, from))) => {
                if let Some(d) = parse_ssdp_response(&buf[..len]) {
                    if !d.st.eq_ignore_ascii_case(&st) {
                        continue;
                    }
                    if seen_usn.insert(d.usn.clone()) {
                        tracing::debug!(
                            "ssdp response from={} usn={} st={} location={} local_ip={:?}",
                            from,
                            d.usn,
                            d.st,
                            d.location,
                            local_ip
                        );
                        devices.push(d);
                    }
                }
            }
            Ok(Err(e)) => return Err(e.into()),
            Err(_) => continue,
        }
    }

    Ok(devices)
}

fn bind_udp_on_iface(local_ip: Ipv4Addr) -> Result<UdpSocket> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_broadcast(true)?;
    // Some platforms return an error for multicast TTL/iface; it's fine to ignore.
    let _ = socket.set_multicast_ttl_v4(2);
    let _ = socket.set_multicast_if_v4(&local_ip);
    socket.bind(&socket2::SockAddr::from(std::net::SocketAddrV4::new(
        local_ip, 0,
    )))?;
    socket.set_nonblocking(true)?;
    let socket: std::net::UdpSocket = socket.into();
    Ok(UdpSocket::from_std(socket)?)
}

fn candidate_ipv4_addrs() -> Vec<Ipv4Addr> {
    let mut out: Vec<Ipv4Addr> = Vec::new();

    let addrs = match get_if_addrs::get_if_addrs() {
        Ok(v) => v,
        Err(_) => return out,
    };

    for iface in addrs {
        let name = iface.name.to_ascii_lowercase();
        // Heuristic: skip common VPN/tunnel adapters so we prefer LAN interfaces for SSDP + HTTP.
        // (When a VPN is enabled on Windows, the default route may point to a tunnel interface.)
        if name.contains("wintun")
            || name.contains("wireguard")
            || name.contains("tailscale")
            || name.contains("zerotier")
            || name.contains("openvpn")
            || name.contains("vpn")
            || name.contains("tap")
            || name.contains("tun")
        {
            continue;
        }
        // Also skip common virtual adapters that often produce private IPs not reachable from LAN
        // devices (WSL/Hyper-V/Docker/VMware/VirtualBox).
        if name.contains("vethernet")
            || name.contains("hyper-v")
            || name.contains("wsl")
            || name.contains("docker")
            || name.contains("vmware")
            || name.contains("virtualbox")
            || name.contains("loopback")
        {
            continue;
        }

        let ip = match iface.ip() {
            IpAddr::V4(v) => v,
            IpAddr::V6(_) => continue,
        };
        if ip.is_loopback() {
            continue;
        }
        // Link-local 169.254.0.0/16
        if ip.octets()[0] == 169 && ip.octets()[1] == 254 {
            continue;
        }
        out.push(ip);
    }

    // Prefer private RFC1918 addresses. If none, return whatever we found.
    let mut private = out
        .iter()
        .copied()
        .filter(|ip| is_private_rfc1918(*ip))
        .collect::<Vec<_>>();
    if !private.is_empty() {
        // Rank common home LAN ranges ahead of other private ranges.
        private.sort_by_key(|ip| (private_ipv4_rank(*ip), *ip));
        private.dedup();
        return private;
    }

    out.sort();
    out.dedup();
    out
}

fn private_ipv4_rank(ip: Ipv4Addr) -> u8 {
    let [a, b, _, _] = ip.octets();
    // Most home routers use 192.168.0.0/16. Many VPN/tunnels also use 10.0.0.0/8.
    // We prefer 192.168 first, then 10, then 172.16/12.
    if a == 192 && b == 168 {
        0
    } else if a == 10 {
        1
    } else if a == 172 && (16..=31).contains(&b) {
        2
    } else {
        3
    }
}

fn is_private_rfc1918(ip: Ipv4Addr) -> bool {
    let [a, b, _, _] = ip.octets();
    a == 10 || (a == 172 && (16..=31).contains(&b)) || (a == 192 && b == 168)
}

fn parse_ssdp_response(bytes: &[u8]) -> Option<DlnaSsdpDevice> {
    let text = String::from_utf8_lossy(bytes);
    let mut lines = text.split("\r\n");
    let status = lines.next()?.trim();
    if !status.starts_with("HTTP/1.1 200") {
        return None;
    }

    let mut usn: Option<String> = None;
    let mut st: Option<String> = None;
    let mut location: Option<String> = None;
    let mut server: Option<String> = None;

    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            break;
        }
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        let key = k.trim().to_ascii_lowercase();
        let value = v.trim().to_string();
        match key.as_str() {
            "usn" => usn = Some(value),
            "st" => st = Some(value),
            "location" => location = Some(value),
            "server" => server = Some(value),
            _ => {}
        }
    }

    Some(DlnaSsdpDevice {
        usn: usn?,
        st: st?,
        location: location?,
        server,
    })
}

// --- Local HTTP server (axum) ---

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

async fn ensure_http_server(
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

fn default_advertise_host() -> Result<String> {
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

fn normalize_ipaddr(ip: std::net::IpAddr) -> String {
    match ip {
        std::net::IpAddr::V4(v4) => v4.to_string(),
        std::net::IpAddr::V6(v6) => format!("[{}]", v6),
    }
}

async fn register_track(path: String) -> String {
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
    use rand::{Rng, rng};
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
    if mime.as_ref() == "application/octet-stream" {
        if let Ok(Some(detected)) = sniff_mime_from_magic(&path).await {
            mime = detected;
        }
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
    let Some(rest) = lower.strip_prefix("bytes=") else {
        return None;
    };
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

// --- SOAP helpers ---

async fn soap_call(
    client: &reqwest::Client,
    control_url: &str,
    service: &str,
    action: &str,
    inner_xml: &str,
) -> Result<String> {
    let envelope = format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\
<s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\">\
<s:Body>\
<u:{action} xmlns:u=\"{service}\">{inner_xml}</u:{action}>\
</s:Body>\
</s:Envelope>"
    );

    let resp = client
        .post(control_url)
        .header("SOAPACTION", format!("\"{}#{}\"", service, action))
        .header("CONTENT-TYPE", "text/xml; charset=\"utf-8\"")
        .body(envelope)
        .send()
        .await?;

    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        anyhow::bail!(
            "soap {}#{} failed status={} body={}",
            service,
            action,
            status,
            truncate(&text, 600)
        );
    }
    Ok(text)
}

fn parse_u8_from_soap(xml: &str, tag_local_name: &str) -> Option<u8> {
    let doc = Document::parse(xml).ok()?;
    let node = doc
        .descendants()
        .find(|n| n.is_element() && n.tag_name().name() == tag_local_name)?;
    let text = node.text()?.trim();
    text.parse::<u8>().ok()
}

async fn rendering_control_try_channels_action(
    client: &reqwest::Client,
    control_url: &str,
    service: &str,
    action: &str,
    build_body: impl Fn(&str) -> String,
) -> Result<()> {
    // Try Master first; many devices support it.
    let channels = ["Master", "LF", "RF"];

    let master_body = build_body(channels[0]);
    soap_call(client, control_url, service, action, &master_body).await?;

    // Best-effort apply to other channels; ignore errors since many devices don't support them.
    for ch in &channels[1..] {
        let body = build_body(ch);
        let _ = soap_call(client, control_url, service, action, &body).await;
    }
    Ok(())
}

fn escape_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

fn ms_to_hhmmss(ms: u64) -> String {
    let total_seconds = ms / 1000;
    let h = total_seconds / 3600;
    let m = (total_seconds % 3600) / 60;
    let s = total_seconds % 60;
    format!("{:02}:{:02}:{:02}", h, m, s)
}

fn hhmmss_to_ms(s: &str) -> Option<u64> {
    let raw = s.trim();
    if raw.is_empty() {
        return None;
    }
    if raw.eq_ignore_ascii_case("not_implemented") || raw.eq_ignore_ascii_case("not implemented") {
        return None;
    }

    let (h, rest) = raw.split_once(':')?;
    let (m, rest) = rest.split_once(':')?;
    let (sec, frac) = match rest.split_once('.') {
        Some((a, b)) => (a, Some(b)),
        None => (rest, None),
    };

    let hh: u64 = h.trim().parse().ok()?;
    let mm: u64 = m.trim().parse().ok()?;
    let ss: u64 = sec.trim().parse().ok()?;

    let mut ms = (hh * 3600 + mm * 60 + ss) * 1000;
    if let Some(frac) = frac {
        let f = frac.trim();
        if !f.is_empty() {
            let digits = f.chars().take(3).collect::<String>();
            if let Ok(v) = digits.parse::<u64>() {
                ms += match digits.len() {
                    1 => v * 100,
                    2 => v * 10,
                    _ => v,
                };
            }
        }
    }
    Some(ms)
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut out = s[..max].to_string();
    out.push_str("…");
    out
}

async fn describe_renderer(
    client: &reqwest::Client,
    device: DlnaSsdpDevice,
) -> Result<Option<DlnaRenderer>> {
    let location = match Url::parse(&device.location) {
        Ok(v) => v,
        Err(e) => {
            tracing::debug!("invalid dlna location url={} err={}", device.location, e);
            return Ok(None);
        }
    };

    let resp = client.get(location.clone()).send().await?;
    if !resp.status().is_success() {
        tracing::debug!(
            "dlna describe non-2xx status={} location={}",
            resp.status(),
            location
        );
        return Ok(None);
    }
    let body = resp.text().await?;

    let doc = match Document::parse(&body) {
        Ok(v) => v,
        Err(e) => {
            tracing::debug!("dlna xml parse failed location={} err={}", location, e);
            return Ok(None);
        }
    };

    let base_url = find_text(&doc, &["URLBase"])
        .and_then(|s| Url::parse(s).ok())
        .unwrap_or_else(|| location.join("/").unwrap_or(location.clone()));

    let friendly_name = find_text(&doc, &["device", "friendlyName"])
        .unwrap_or("DLNA Renderer")
        .trim()
        .to_string();

    let mut av_transport_control_url: Option<String> = None;
    let mut av_transport_service_type: Option<String> = None;
    let mut rendering_control_url: Option<String> = None;
    let mut rendering_control_service_type: Option<String> = None;

    for service in find_services(&doc) {
        let Some(service_type) = find_text_node(service, "serviceType").and_then(|t| t.text())
        else {
            continue;
        };
        let Some(control_url_raw) = find_text_node(service, "controlURL").and_then(|t| t.text())
        else {
            continue;
        };

        let control_url_raw = control_url_raw.trim();
        let abs = match Url::parse(control_url_raw) {
            Ok(v) => v,
            Err(_) => match base_url.join(control_url_raw) {
                Ok(v) => v,
                Err(_) => continue,
            },
        };

        let service_type_trimmed = service_type.trim();
        if av_transport_control_url.is_none() && service_type_trimmed.starts_with(ST_AV_TRANSPORT) {
            av_transport_control_url = Some(abs.to_string());
            av_transport_service_type = Some(service_type_trimmed.to_string());
        } else if rendering_control_url.is_none()
            && service_type_trimmed.starts_with(ST_RENDERING_CONTROL)
        {
            rendering_control_url = Some(abs.to_string());
            rendering_control_service_type = Some(service_type_trimmed.to_string());
        }

        if av_transport_control_url.is_some() && rendering_control_url.is_some() {
            break;
        }
    }

    Ok(Some(DlnaRenderer {
        usn: device.usn,
        location: device.location,
        friendly_name,
        av_transport_control_url,
        av_transport_service_type,
        rendering_control_url,
        rendering_control_service_type,
    }))
}

fn find_text<'a>(doc: &'a Document<'a>, path: &[&str]) -> Option<&'a str> {
    let mut node = doc.root_element();
    for name in path {
        node = node
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == *name)?;
    }
    node.text()
}

fn soap_get_text(xml: &str, tag_local_name: &str) -> Option<String> {
    let doc = Document::parse(xml).ok()?;
    let node = doc
        .descendants()
        .find(|n| n.is_element() && n.tag_name().name() == tag_local_name)?;
    Some(node.text()?.trim().to_string())
}

fn find_services<'a>(doc: &'a Document<'a>) -> impl Iterator<Item = roxmltree::Node<'a, 'a>> {
    doc.descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "service")
}

fn find_text_node<'a>(
    service: roxmltree::Node<'a, 'a>,
    name: &str,
) -> Option<roxmltree::Node<'a, 'a>> {
    service
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == name)
}
