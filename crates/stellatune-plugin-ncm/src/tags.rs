use base64::Engine;
use symphonia::core::meta::{StandardTagKey, StandardVisualKey, Value};

#[derive(Default)]
pub(crate) struct Tags {
    pub(crate) title: Option<String>,
    pub(crate) artist: Option<String>,
    pub(crate) album: Option<String>,
    pub(crate) cover: Option<Vec<u8>>,
}

pub(crate) fn apply_revision(rev: &symphonia::core::meta::MetadataRevision, tags: &mut Tags) {
    for tag in rev.tags() {
        if tags.title.is_none() && matches!(tag.std_key, Some(StandardTagKey::TrackTitle)) {
            tags.title = value_to_string(&tag.value);
            continue;
        }
        if tags.artist.is_none() && matches!(tag.std_key, Some(StandardTagKey::Artist)) {
            tags.artist = value_to_string(&tag.value);
            continue;
        }
        if tags.album.is_none() && matches!(tag.std_key, Some(StandardTagKey::Album)) {
            tags.album = value_to_string(&tag.value);
            continue;
        }
    }

    if tags.cover.is_none() {
        let front = rev
            .visuals()
            .iter()
            .find(|v| v.usage == Some(StandardVisualKey::FrontCover));
        let any = rev.visuals().first();
        let chosen = front.or(any);
        if let Some(bytes) =
            chosen.and_then(|v| (!v.data.is_empty()).then(|| v.data.as_ref().to_vec()))
        {
            tags.cover = Some(bytes);
        }
    }
}

fn value_to_string(v: &Value) -> Option<String> {
    let s = match v {
        Value::String(s) => s.clone(),
        _ => v.to_string(),
    };
    let s = s.trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

pub(crate) fn build_metadata_json(tags: Tags, duration_ms: Option<u64>) -> Option<String> {
    let mut obj = serde_json::Map::<String, serde_json::Value>::new();
    if let Some(s) = tags.title {
        obj.insert("title".to_string(), serde_json::Value::String(s));
    }
    if let Some(s) = tags.artist {
        obj.insert("artist".to_string(), serde_json::Value::String(s));
    }
    if let Some(s) = tags.album {
        obj.insert("album".to_string(), serde_json::Value::String(s));
    }
    if let Some(ms) = duration_ms {
        obj.insert(
            "duration_ms".to_string(),
            serde_json::Value::Number(serde_json::Number::from(ms)),
        );
    }
    if let Some(b64) = tags.cover.and_then(|bytes| {
        (!bytes.is_empty()).then(|| base64::engine::general_purpose::STANDARD.encode(bytes))
    }) {
        obj.insert("cover_base64".to_string(), serde_json::Value::String(b64));
    }
    if obj.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(obj).to_string())
    }
}
