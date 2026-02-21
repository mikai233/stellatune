use std::sync::OnceLock;
use std::thread;
use std::time::{Duration, Instant};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use stellatune_plugin_sdk::__private::parking_lot::{Mutex, MutexGuard};
use stellatune_plugin_sdk::__private::stellatune_world_source::stellatune::plugin::{
    host_stream, sidecar,
};
use stellatune_plugin_sdk::prelude::*;
use url::Url;

const DEFAULT_SIDECAR_BASE_URL: &str = "http://127.0.0.1:46321";
const DEFAULT_SIDECAR_EXECUTABLE: &str = "stellatune-ncm-sidecar";
const DEFAULT_REQUEST_TIMEOUT_MS: u64 = 8_000;
const DEFAULT_LEVEL: &str = "standard";
const SIDECAR_READY_TIMEOUT_SECS: u64 = 10;
const SIDECAR_READY_POLL_MS: u64 = 150;
const MAX_HTTP_BODY_BYTES: usize = 16 * 1024 * 1024;

pub const SOURCE_TYPE_ID: &str = "netease";
pub const SOURCE_DISPLAY_NAME: &str = "Netease Cloud Music";

pub const CONFIG_SCHEMA_JSON: &str = r#"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "additionalProperties": false,
  "properties": {
    "sidecar_base_url": { "type": "string", "default": "http://127.0.0.1:46321" },
    "sidecar_path": { "type": ["string", "null"] },
    "sidecar_args": {
      "type": "array",
      "items": { "type": "string" },
      "default": []
    },
    "request_timeout_ms": { "type": "integer", "minimum": 500, "default": 8000 },
    "default_level": { "type": "string", "default": "standard" }
  }
}"#;

pub const DEFAULT_CONFIG_JSON: &str = r#"{
  "sidecar_base_url": "http://127.0.0.1:46321",
  "sidecar_path": null,
  "sidecar_args": [],
  "request_timeout_ms": 8000,
  "default_level": "standard"
}"#;

static SIDECAR_MANAGER: OnceLock<Mutex<SidecarManager>> = OnceLock::new();

#[derive(Default)]
struct SidecarManager {
    signature: Option<String>,
    process: Option<sidecar::Process>,
}

impl SidecarManager {
    fn ensure_for(&mut self, config: &NeteaseSourceConfig) -> SdkResult<()> {
        let signature = config_signature(config);
        let needs_restart = self
            .signature
            .as_deref()
            .map(|current| current != signature)
            .unwrap_or(true)
            || self.process.is_none();

        if !needs_restart {
            return Ok(());
        }

        self.shutdown_current();
        self.process = Some(launch_sidecar_process(config)?);
        self.signature = Some(signature.to_string());
        Ok(())
    }

    fn shutdown_current(&mut self) {
        if let Some(process) = self.process.take() {
            let _ = process.terminate(200);
            let _ = process.wait_exit(Some(500));
        }
        self.signature = None;
    }
}

fn sidecar_manager() -> &'static Mutex<SidecarManager> {
    SIDECAR_MANAGER.get_or_init(|| Mutex::new(SidecarManager::default()))
}

fn lock_sidecar_manager() -> MutexGuard<'static, SidecarManager> {
    sidecar_manager().lock()
}

pub fn shutdown_sidecar_manager() {
    let mut manager = lock_sidecar_manager();
    manager.shutdown_current();
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NeteaseSourceConfig {
    pub sidecar_base_url: String,
    pub sidecar_path: Option<String>,
    pub sidecar_args: Vec<String>,
    pub request_timeout_ms: u64,
    pub default_level: String,
}

impl Default for NeteaseSourceConfig {
    fn default() -> Self {
        Self {
            sidecar_base_url: DEFAULT_SIDECAR_BASE_URL.to_string(),
            sidecar_path: None,
            sidecar_args: Vec::new(),
            request_timeout_ms: DEFAULT_REQUEST_TIMEOUT_MS,
            default_level: DEFAULT_LEVEL.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct NeteaseListRequest {
    action: String,
    keywords: String,
    playlist_id: Option<u64>,
    playlist_ref: Option<Value>,
    limit: u32,
    offset: u32,
    level: Option<String>,
}

impl Default for NeteaseListRequest {
    fn default() -> Self {
        Self {
            action: "search".to_string(),
            keywords: String::new(),
            playlist_id: None,
            playlist_ref: None,
            limit: 30,
            offset: 0,
            level: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NeteaseTrack {
    song_id: u64,
    level: String,
    stream_url: Option<String>,
    ext_hint: Option<String>,
    cover: Option<SidecarCoverRef>,
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    duration_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NeteaseListItem {
    kind: String,
    item_id: String,
    source_id: String,
    source_label: Option<String>,
    track_id: Option<String>,
    playlist_id: Option<String>,
    title: String,
    subtitle: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    duration_ms: Option<i64>,
    track_count: Option<i64>,
    cover: Option<SidecarCoverRef>,
    ext_hint: Option<String>,
    path_hint: Option<String>,
    playlist_ref: Option<Value>,
    track: Option<NeteaseTrack>,
}

#[derive(Debug, Deserialize)]
struct SidecarHealthResponse {
    ok: bool,
}

#[derive(Debug, Deserialize)]
struct SidecarSongListResponse {
    items: Vec<SidecarSongItem>,
}

#[derive(Debug, Deserialize)]
struct SidecarSongItem {
    song_id: u64,
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    duration_ms: Option<i64>,
    ext_hint: Option<String>,
    cover: Option<SidecarCoverRef>,
    stream_url: Option<String>,
    level: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SidecarSongUrlResponse {
    url: String,
    ext_hint: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SidecarPlaylistListResponse {
    items: Vec<SidecarPlaylistItem>,
}

#[derive(Debug, Deserialize)]
struct SidecarPlaylistItem {
    playlist_id: String,
    title: String,
    track_count: Option<i64>,
    cover: Option<SidecarCoverRef>,
    source_id: Option<String>,
    source_label: Option<String>,
    playlist_ref: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SidecarCoverRef {
    kind: String,
    value: String,
    mime: Option<String>,
}

pub struct NeteaseSourceCatalog {
    config: NeteaseSourceConfig,
}

impl NeteaseSourceCatalog {
    pub fn new() -> Self {
        Self {
            config: NeteaseSourceConfig::default(),
        }
    }
}

impl ConfigStateOps for NeteaseSourceCatalog {
    fn apply_config_update_json(&mut self, new_config_json: &str) -> SdkResult<()> {
        let next: NeteaseSourceConfig = serde_json::from_str(new_config_json).map_err(|error| {
            SdkError::invalid_arg(format!("invalid source config JSON: {error}"))
        })?;
        self.config = next;
        Ok(())
    }
}

pub struct UnsupportedSourceStream;

impl SourceStream for UnsupportedSourceStream {
    fn metadata(&self) -> SdkResult<MediaMetadata> {
        Err(SdkError::unsupported(
            "processed source stream is unsupported for netease source",
        ))
    }

    fn read(&mut self, _max_bytes: u32) -> SdkResult<EncodedChunk> {
        Err(SdkError::unsupported(
            "processed source stream is unsupported for netease source",
        ))
    }
}

impl SourceCatalog for NeteaseSourceCatalog {
    type Stream = UnsupportedSourceStream;

    fn list_items_json(&mut self, request_json: &str) -> SdkResult<String> {
        let request: NeteaseListRequest = serde_json::from_str(request_json).map_err(|error| {
            SdkError::invalid_arg(format!("invalid list request JSON: {error}"))
        })?;
        let items = list_items(&self.config, &request)?;
        serde_json::to_string(&items)
            .map_err(|error| SdkError::internal(format!("serialize list items JSON: {error}")))
    }

    fn open_stream_json(&mut self, _track_json: &str) -> SdkResult<Self::Stream> {
        Err(SdkError::unsupported(
            "source.open-stream-json requires opened-stream passthrough mode",
        ))
    }

    fn open_stream_opened_json(
        &mut self,
        track_json: &str,
    ) -> SdkResult<OpenedSourceStream<Self::Stream>> {
        let track: NeteaseTrack = serde_json::from_str(track_json)
            .map_err(|error| SdkError::invalid_arg(format!("invalid track JSON: {error}")))?;
        ensure_sidecar_running(&self.config)?;
        let resolved = resolve_stream_url(&self.config, &track)?;

        let ext_hint = guess_ext_hint(&resolved.url, resolved.ext_hint.as_deref())
            .or_else(|| normalized_lower_text(track.ext_hint.as_deref()))
            .unwrap_or_else(|| "mp3".to_string());

        let metadata = build_track_metadata(&track, ext_hint.as_str());
        let mut request = HostStreamOpenRequest::http(resolved.url);
        let timeout = clamp_timeout_ms(self.config.request_timeout_ms);
        request.connect_timeout_ms = Some(timeout);
        request.read_timeout_ms = Some(timeout);

        Ok(OpenedSourceStream::PassthroughRequest {
            request,
            ext_hint: Some(ext_hint),
            metadata: Some(metadata),
        })
    }
}

fn list_items(
    config: &NeteaseSourceConfig,
    request: &NeteaseListRequest,
) -> SdkResult<Vec<NeteaseListItem>> {
    if request.action.trim().eq_ignore_ascii_case("list_playlists") {
        return fetch_playlist_items(config, request);
    }

    let action = request.action.trim().to_ascii_lowercase();
    if action == "ensure_sidecar" {
        ensure_sidecar_running(config)?;
        return Ok(Vec::new());
    }
    if action == "shutdown_sidecar" {
        shutdown_sidecar(config);
        return Ok(Vec::new());
    }

    let songs = fetch_song_items(config, request)?;
    let level = normalize_level(request.level.as_deref(), config);
    Ok(songs
        .into_iter()
        .map(|item| {
            let song_id = item.song_id;
            let title =
                non_empty_text(item.title.as_deref()).unwrap_or_else(|| format!("Song {song_id}"));
            let ext_hint = normalize_ext_hint(item.ext_hint.as_deref());
            let path_hint = format!("netease:{song_id}.{ext_hint}");
            let track = NeteaseTrack {
                song_id,
                level: item.level.unwrap_or_else(|| level.clone()),
                stream_url: item.stream_url,
                ext_hint: Some(ext_hint.clone()),
                cover: item.cover.clone(),
                title: item.title.clone(),
                artist: item.artist.clone(),
                album: item.album.clone(),
                duration_ms: item.duration_ms,
            };
            NeteaseListItem {
                kind: "track".to_string(),
                item_id: song_id.to_string(),
                source_id: SOURCE_TYPE_ID.to_string(),
                source_label: Some(SOURCE_DISPLAY_NAME.to_string()),
                track_id: Some(song_id.to_string()),
                playlist_id: None,
                title,
                subtitle: None,
                artist: item.artist,
                album: item.album,
                duration_ms: item.duration_ms,
                track_count: None,
                cover: item.cover,
                ext_hint: Some(ext_hint),
                path_hint: Some(path_hint),
                playlist_ref: None,
                track: Some(track),
            }
        })
        .collect())
}

fn fetch_song_items(
    config: &NeteaseSourceConfig,
    request: &NeteaseListRequest,
) -> SdkResult<Vec<SidecarSongItem>> {
    let limit = request.limit.clamp(1, 1000);
    let offset = request.offset;
    let action = request.action.trim().to_ascii_lowercase();

    if action == "playlist_tracks" {
        let playlist_id = extract_playlist_id(request)
            .ok_or_else(|| SdkError::invalid_arg("playlist_id is required"))?;
        let params = vec![
            ("playlist_id".to_string(), playlist_id.to_string()),
            ("limit".to_string(), limit.to_string()),
            ("offset".to_string(), offset.to_string()),
            (
                "level".to_string(),
                normalize_level(request.level.as_deref(), config),
            ),
        ];
        let response: SidecarSongListResponse =
            sidecar_get_json(config, "/v1/playlist/tracks", &params)?;
        return Ok(response.items);
    }

    let keywords = request.keywords.trim();
    if keywords.is_empty() {
        return Ok(Vec::new());
    }
    let params = vec![
        ("keywords".to_string(), keywords.to_string()),
        ("limit".to_string(), limit.to_string()),
        ("offset".to_string(), offset.to_string()),
        (
            "level".to_string(),
            normalize_level(request.level.as_deref(), config),
        ),
    ];
    let response: SidecarSongListResponse = sidecar_get_json(config, "/v1/search", &params)?;
    Ok(response.items)
}

fn fetch_playlist_items(
    config: &NeteaseSourceConfig,
    request: &NeteaseListRequest,
) -> SdkResult<Vec<NeteaseListItem>> {
    ensure_sidecar_running(config)?;
    let limit = request.limit.clamp(1, 200);
    let offset = request.offset;
    let params = vec![
        ("limit".to_string(), limit.to_string()),
        ("offset".to_string(), offset.to_string()),
        ("source_label".to_string(), SOURCE_DISPLAY_NAME.to_string()),
    ];
    let response: SidecarPlaylistListResponse = sidecar_get_json(config, "/v1/playlists", &params)?;
    Ok(response
        .items
        .into_iter()
        .map(|item| NeteaseListItem {
            kind: "playlist".to_string(),
            item_id: item.playlist_id.clone(),
            source_id: item.source_id.unwrap_or_else(|| SOURCE_TYPE_ID.to_string()),
            source_label: item.source_label,
            track_id: None,
            playlist_id: Some(item.playlist_id.clone()),
            title: item.title,
            subtitle: None,
            artist: None,
            album: None,
            duration_ms: None,
            track_count: item.track_count,
            cover: item.cover,
            ext_hint: None,
            path_hint: None,
            playlist_ref: item.playlist_ref.or_else(|| {
                item.playlist_id.parse::<u64>().ok().map(|id| {
                    let mut map = serde_json::Map::new();
                    map.insert("playlist_id".to_string(), Value::Number(id.into()));
                    Value::Object(map)
                })
            }),
            track: None,
        })
        .collect())
}

fn extract_playlist_id(request: &NeteaseListRequest) -> Option<u64> {
    if let Some(v) = request.playlist_id {
        return Some(v);
    }
    let from_ref = request
        .playlist_ref
        .as_ref()
        .and_then(|v| v.as_object())
        .and_then(|obj| obj.get("playlist_id"))
        .and_then(value_to_u64);
    if from_ref.is_some() {
        return from_ref;
    }
    request
        .playlist_ref
        .as_ref()
        .and_then(|v| v.as_object())
        .and_then(|obj| obj.get("id"))
        .and_then(value_to_u64)
}

fn value_to_u64(value: &Value) -> Option<u64> {
    if let Some(v) = value.as_u64() {
        return Some(v);
    }
    if let Some(v) = value.as_i64() {
        return u64::try_from(v).ok();
    }
    value.as_str().and_then(|s| s.trim().parse::<u64>().ok())
}

fn resolve_stream_url(
    config: &NeteaseSourceConfig,
    track: &NeteaseTrack,
) -> SdkResult<SidecarSongUrlResponse> {
    if let Some(url) = non_empty_text(track.stream_url.as_deref()) {
        return Ok(SidecarSongUrlResponse {
            url,
            ext_hint: track.ext_hint.clone(),
        });
    }

    let params = vec![
        ("song_id".to_string(), track.song_id.to_string()),
        (
            "level".to_string(),
            normalize_level(Some(&track.level), config),
        ),
    ];
    sidecar_get_json(config, "/v1/song/url", &params)
}

fn sidecar_get_json<T: DeserializeOwned>(
    config: &NeteaseSourceConfig,
    path: &str,
    params: &[(String, String)],
) -> SdkResult<T> {
    ensure_sidecar_running(config)?;
    let url = build_url(config, path, params)?;
    let payload = http_get_bytes(&url, clamp_timeout_ms(config.request_timeout_ms))?;
    serde_json::from_slice::<T>(&payload).map_err(|error| {
        SdkError::internal(format!("decode sidecar JSON failed path={path}: {error}"))
    })
}

fn build_url(
    config: &NeteaseSourceConfig,
    path: &str,
    params: &[(String, String)],
) -> SdkResult<String> {
    let base = normalize_base_url(&config.sidecar_base_url);
    let mut url = Url::parse(format!("{base}{path}").as_str())
        .map_err(|error| SdkError::invalid_arg(format!("invalid sidecar base URL: {error}")))?;
    {
        let mut query = url.query_pairs_mut();
        for (key, value) in params {
            query.append_pair(key.as_str(), value.as_str());
        }
    }
    Ok(url.to_string())
}

fn http_get_bytes(url: &str, timeout_ms: u32) -> SdkResult<Vec<u8>> {
    let request = host_stream::OpenRequest {
        kind: host_stream::StreamOpenKind::Http,
        target: url.to_string(),
        method: Some(host_stream::HttpMethod::Get),
        headers: Vec::new(),
        body: None,
        connect_timeout_ms: Some(timeout_ms.max(500)),
        read_timeout_ms: Some(timeout_ms.max(500)),
    };
    let handle = host_stream::open(&request).map_err(map_host_stream_error)?;

    let mut out = Vec::<u8>::new();
    loop {
        let chunk = handle.read(64 * 1024).map_err(map_host_stream_error)?;
        if chunk.is_empty() {
            break;
        }
        if out.len().saturating_add(chunk.len()) > MAX_HTTP_BODY_BYTES {
            let _ = handle.close();
            return Err(SdkError::io(format!(
                "HTTP response too large (> {} bytes)",
                MAX_HTTP_BODY_BYTES
            )));
        }
        out.extend_from_slice(&chunk);
    }
    let _ = handle.close();
    Ok(out)
}

fn ensure_sidecar_running(config: &NeteaseSourceConfig) -> SdkResult<()> {
    if sidecar_health_ok(config) {
        return Ok(());
    }

    {
        let mut manager = lock_sidecar_manager();
        manager.ensure_for(config)?;
    }

    let deadline = Instant::now() + Duration::from_secs(SIDECAR_READY_TIMEOUT_SECS);
    while Instant::now() < deadline {
        if sidecar_health_ok(config) {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(SIDECAR_READY_POLL_MS));
    }
    Err(SdkError::timeout(
        "netease sidecar did not become ready in time",
    ))
}

fn sidecar_health_ok(config: &NeteaseSourceConfig) -> bool {
    let params: Vec<(String, String)> = Vec::new();
    sidecar_get_json_without_bootstrap::<SidecarHealthResponse>(config, "/health", &params)
        .map(|response| response.ok)
        .unwrap_or(false)
}

fn sidecar_get_json_without_bootstrap<T: DeserializeOwned>(
    config: &NeteaseSourceConfig,
    path: &str,
    params: &[(String, String)],
) -> SdkResult<T> {
    let url = build_url(config, path, params)?;
    let payload = http_get_bytes(&url, clamp_timeout_ms(config.request_timeout_ms))?;
    serde_json::from_slice::<T>(&payload).map_err(|error| {
        SdkError::internal(format!("decode sidecar JSON failed path={path}: {error}"))
    })
}

fn shutdown_sidecar(config: &NeteaseSourceConfig) {
    let params: Vec<(String, String)> = Vec::new();
    let _ = sidecar_get_json_without_bootstrap::<Value>(config, "/v1/admin/shutdown", &params);
    shutdown_sidecar_manager();
}

fn launch_sidecar_process(config: &NeteaseSourceConfig) -> SdkResult<sidecar::Process> {
    let executable = config
        .sidecar_path
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or(DEFAULT_SIDECAR_EXECUTABLE)
        .to_string();

    let spec = sidecar::LaunchSpec {
        scope: sidecar::LaunchScope::PackageShared,
        executable,
        args: config.sidecar_args.clone(),
        preferred_control: vec![sidecar::TransportOption {
            kind: sidecar::TransportKind::Stdio,
            priority: 10,
            max_frame_bytes: None,
        }],
        preferred_data: Vec::new(),
        env: Vec::new(),
    };
    sidecar::launch(&spec).map_err(map_sidecar_error)
}

fn map_host_stream_error(error: host_stream::PluginError) -> SdkError {
    match error {
        host_stream::PluginError::InvalidArg(message) => SdkError::invalid_arg(message),
        host_stream::PluginError::NotFound(message) => SdkError::not_found(message),
        host_stream::PluginError::Io(message) => SdkError::io(message),
        host_stream::PluginError::Timeout(message) => SdkError::timeout(message),
        host_stream::PluginError::Unsupported(message) => SdkError::unsupported(message),
        host_stream::PluginError::Denied(message) => SdkError::denied(message),
        host_stream::PluginError::Internal(message) => SdkError::internal(message),
    }
}

fn map_sidecar_error(error: sidecar::PluginError) -> SdkError {
    match error {
        sidecar::PluginError::InvalidArg(message) => SdkError::invalid_arg(message),
        sidecar::PluginError::NotFound(message) => SdkError::not_found(message),
        sidecar::PluginError::Io(message) => SdkError::io(message),
        sidecar::PluginError::Timeout(message) => SdkError::timeout(message),
        sidecar::PluginError::Unsupported(message) => SdkError::unsupported(message),
        sidecar::PluginError::Denied(message) => SdkError::denied(message),
        sidecar::PluginError::Internal(message) => SdkError::internal(message),
    }
}

fn config_signature(config: &NeteaseSourceConfig) -> String {
    let path = config
        .sidecar_path
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or(DEFAULT_SIDECAR_EXECUTABLE);
    let args = config.sidecar_args.join("\u{1f}");
    let base = normalize_base_url(config.sidecar_base_url.as_str());
    format!("{path}\u{1e}{args}\u{1e}{base}")
}

fn build_track_metadata(track: &NeteaseTrack, codec: &str) -> MediaMetadata {
    let artists = track.artist.clone().into_iter().collect::<Vec<_>>();
    MediaMetadata {
        tags: AudioTags {
            title: track.title.clone(),
            album: track.album.clone(),
            artists,
            album_artists: Vec::new(),
            genres: Vec::new(),
            track_number: None,
            track_total: None,
            disc_number: None,
            disc_total: None,
            year: None,
            comment: None,
        },
        duration_ms: track.duration_ms.and_then(|ms| u64::try_from(ms).ok()),
        format: EncodedAudioFormat {
            codec: codec.to_string(),
            sample_rate: None,
            channels: None,
            bitrate_kbps: None,
            container: None,
        },
        extras: vec![MetadataEntry {
            key: "song_id".to_string(),
            value: MetadataValue::Uint64(track.song_id),
        }],
    }
}

fn clamp_timeout_ms(timeout_ms: u64) -> u32 {
    timeout_ms.clamp(500, u32::MAX as u64) as u32
}

fn normalize_base_url(raw: &str) -> String {
    let value = raw.trim();
    if value.is_empty() {
        return DEFAULT_SIDECAR_BASE_URL.to_string();
    }
    value.trim_end_matches('/').to_string()
}

fn normalize_level(raw: Option<&str>, config: &NeteaseSourceConfig) -> String {
    raw.map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToString::to_string)
        .or_else(|| {
            let cfg = config.default_level.trim();
            (!cfg.is_empty()).then(|| cfg.to_string())
        })
        .unwrap_or_else(|| DEFAULT_LEVEL.to_string())
        .to_ascii_lowercase()
}

fn normalize_ext_hint(raw: Option<&str>) -> String {
    normalized_lower_text(raw).unwrap_or_else(|| "mp3".to_string())
}

fn non_empty_text(raw: Option<&str>) -> Option<String> {
    raw.map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToString::to_string)
}

fn normalized_lower_text(raw: Option<&str>) -> Option<String> {
    raw.map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| v.to_ascii_lowercase())
}

fn guess_ext_hint(stream_url: &str, fallback: Option<&str>) -> Option<String> {
    if let Ok(url) = Url::parse(stream_url)
        && let Some(path) = url.path_segments().and_then(|mut segs| segs.next_back())
        && let Some(ext) = std::path::Path::new(path)
            .extension()
            .and_then(|v| v.to_str())
    {
        let ext = ext.trim().to_ascii_lowercase();
        if !ext.is_empty() {
            return Some(ext);
        }
    }
    normalized_lower_text(fallback)
}
