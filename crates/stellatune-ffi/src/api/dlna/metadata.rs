use anyhow::Result;
use mime_guess::MimeGuess;
use roxmltree::Document;
use url::Url;

use super::transport::escape_xml;
use super::types::{DlnaRenderer, DlnaSsdpDevice};

const ST_AV_TRANSPORT: &str = "urn:schemas-upnp-org:service:AVTransport:";
const ST_RENDERING_CONTROL: &str = "urn:schemas-upnp-org:service:RenderingControl:";

pub(super) fn build_didl_metadata(
    track_url: &str,
    track_path: &str,
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    cover_url: Option<&str>,
) -> String {
    let fallback_title = track_path.rsplit(['/', '\\']).next().unwrap_or(track_path);
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

pub(super) async fn describe_renderer(
    client: &reqwest::Client,
    device: DlnaSsdpDevice,
) -> Result<Option<DlnaRenderer>> {
    let location = match Url::parse(&device.location) {
        Ok(v) => v,
        Err(e) => {
            tracing::debug!("invalid dlna location url={} err={}", device.location, e);
            return Ok(None);
        },
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
        },
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

pub(super) fn soap_get_text(xml: &str, tag_local_name: &str) -> Option<String> {
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
