use std::net::SocketAddr;
use std::time::Duration;

use super::types::{
    DlnaHttpServerInfo, DlnaPositionInfo, DlnaRenderer, DlnaSsdpDevice, DlnaTransportInfo,
};
use anyhow::Result;

use super::{discovery, http_server, metadata};
use roxmltree::Document;
use tokio::task::JoinSet;

const ST_MEDIA_RENDERER: &str = "urn:schemas-upnp-org:device:MediaRenderer:1";
const SERVICE_AV_TRANSPORT_1: &str = "urn:schemas-upnp-org:service:AVTransport:1";
const SERVICE_RENDERING_CONTROL_1: &str = "urn:schemas-upnp-org:service:RenderingControl:1";

pub(crate) async fn discover_media_renderers(timeout_ms: u32) -> Result<Vec<DlnaSsdpDevice>> {
    let timeout = Duration::from_millis(timeout_ms.max(200) as u64);
    discovery::ssdp_msearch_multi_iface(ST_MEDIA_RENDERER, 1, timeout).await
}

pub(crate) async fn discover_renderers(timeout_ms: u32) -> Result<Vec<DlnaRenderer>> {
    let devices = discover_media_renderers(timeout_ms).await?;
    if devices.is_empty() {
        return Ok(Vec::new());
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(timeout_ms.max(2000) as u64))
        .build()?;

    let mut join = JoinSet::new();
    for d in devices {
        let client = client.clone();
        join.spawn(async move { metadata::describe_renderer(&client, d).await });
    }

    let mut out = Vec::new();
    while let Some(res) = join.join_next().await {
        match res {
            Ok(Ok(Some(renderer))) => out.push(renderer),
            Ok(Ok(None)) => {},
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
    http_server::ensure_http_server(advertise_ip, port).await
}

pub(crate) async fn http_publish_track(path: String) -> Result<String> {
    let info = http_server::ensure_http_server(None, None).await?;
    let token = http_server::register_track(path).await;
    // Even if the HTTP server is already running, compute the advertised host at publish-time.
    // This prevents a VPN/tunnel default route change from making the previously chosen
    // `base_url` unreachable from the DLNA renderer.
    let listen_addr: SocketAddr = info.listen_addr.parse()?;
    let host = http_server::default_advertise_host()?;
    let url = format!("http://{}:{}/track/{}", host, listen_addr.port(), token);
    tracing::info!("dlna publish track url={}", url);
    Ok(url)
}

pub(crate) async fn http_unpublish_all() -> Result<()> {
    http_server::unpublish_all().await;
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

    let state = metadata::soap_get_text(&xml, "CurrentTransportState")
        .unwrap_or_else(|| "UNKNOWN".to_string());
    let status = metadata::soap_get_text(&xml, "CurrentTransportStatus");
    let speed = metadata::soap_get_text(&xml, "CurrentSpeed");
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

    let rel_time_ms = metadata::soap_get_text(&xml, "RelTime")
        .as_deref()
        .and_then(hhmmss_to_ms)
        .unwrap_or(0);
    let track_duration_ms = metadata::soap_get_text(&xml, "TrackDuration")
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
    rendering_control_try_channels_action(&client, &control_url, service, "SetVolume", |channel| {
        format!(
            "<InstanceID>0</InstanceID>\
<Channel>{}</Channel>\
<DesiredVolume>{}</DesiredVolume>",
            escape_xml(channel),
            volume_0_100
        )
    })
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
    rendering_control_try_channels_action(&client, &control_url, service, "SetMute", |channel| {
        format!(
            "<InstanceID>0</InstanceID>\
<Channel>{}</Channel>\
<DesiredMute>{}</DesiredMute>",
            escape_xml(channel),
            desired
        )
    })
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
    play_local_track(renderer, path, None, None, None, None).await
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

    let url = http_publish_track(path.clone()).await?;

    let cover_url = if let Some(cp) = cover_path {
        match tokio::fs::metadata(&cp).await {
            Ok(_) => Some(http_publish_track(cp).await?),
            Err(_) => None,
        }
    } else {
        None
    };

    let meta =
        metadata::build_didl_metadata(&url, &path, title, artist, album, cover_url.as_deref());

    av_transport_set_uri(
        control_url.clone(),
        service_type.clone(),
        url.clone(),
        Some(meta),
    )
    .await?;
    av_transport_play(control_url, service_type).await?;
    Ok(url)
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

pub(super) fn escape_xml(s: &str) -> String {
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
    out.push('…');
    out
}
