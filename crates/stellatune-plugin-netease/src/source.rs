use std::io::{Cursor, Read as _, Seek as _, SeekFrom};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use reqwest::Url;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use stellatune_plugin_sdk::instance::{
    SourceCatalogDescriptor, SourceCatalogInstance, SourceOpenResult, SourceStream,
};
use stellatune_plugin_sdk::update::ConfigUpdatable;
use stellatune_plugin_sdk::{
    SdkError, SdkResult, StLogLevel, StSeekWhence, host_log, resolve_runtime_path, sidecar_command,
};
use tokio::time::sleep;

const DEFAULT_SIDECAR_BASE_URL: &str = "http://127.0.0.1:46321";
const DEFAULT_REQUEST_TIMEOUT_MS: u64 = 8_000;
const DEFAULT_LEVEL: &str = "standard";
const SIDECAR_READY_TIMEOUT_SECS: u64 = 10;
const SIDECAR_HEALTH_TIMEOUT_MS: u64 = 1_200;
const SIDECAR_START_RETRY_BASE_COOLDOWN_MS: u64 = 2_000;
const SIDECAR_START_RETRY_MAX_COOLDOWN_MS: u64 = 30_000;
const SOURCE_TYPE_ID: &str = "netease";
const SOURCE_DISPLAY_NAME: &str = "Netease Cloud Music";

static SIDECAR_START_LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
static SIDECAR_START_STATE: OnceLock<Mutex<SidecarStartState>> = OnceLock::new();

#[derive(Debug, Default)]
struct SidecarStartState {
    consecutive_failures: u32,
    last_failure_at: Option<Instant>,
}

impl SidecarStartState {
    fn mark_success(&mut self) {
        self.consecutive_failures = 0;
        self.last_failure_at = None;
    }

    fn mark_failure(&mut self) {
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);
        self.last_failure_at = Some(Instant::now());
    }

    fn current_cooldown(&self) -> Duration {
        let exp = self.consecutive_failures.saturating_sub(1).min(4u32);
        let factor = 1u64.checked_shl(exp).unwrap_or(u64::MAX);
        let cooldown_ms = SIDECAR_START_RETRY_BASE_COOLDOWN_MS
            .saturating_mul(factor)
            .min(SIDECAR_START_RETRY_MAX_COOLDOWN_MS);
        Duration::from_millis(cooldown_ms)
    }

    fn remaining_cooldown(&self) -> Option<Duration> {
        let failed_at = self.last_failure_at?;
        let cooldown = self.current_cooldown();
        let elapsed = failed_at.elapsed();
        (elapsed < cooldown).then(|| cooldown - elapsed)
    }
}

const CONFIG_SCHEMA_JSON: &str = r#"{
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
pub struct NeteaseListRequest {
    pub action: String,
    pub keywords: String,
    pub playlist_id: Option<u64>,
    pub playlist_ref: Option<Value>,
    pub limit: u32,
    pub offset: u32,
    pub level: Option<String>,
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
pub struct NeteaseTrack {
    pub song_id: u64,
    pub level: String,
    pub stream_url: Option<String>,
    pub ext_hint: Option<String>,
    pub cover: Option<SidecarCoverRef>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeteaseListItem {
    pub kind: String,
    pub item_id: String,
    pub source_id: String,
    pub source_label: Option<String>,
    pub track_id: Option<String>,
    pub playlist_id: Option<String>,
    pub title: String,
    pub subtitle: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_ms: Option<i64>,
    pub track_count: Option<i64>,
    pub cover: Option<SidecarCoverRef>,
    pub ext_hint: Option<String>,
    pub path_hint: Option<String>,
    pub playlist_ref: Option<Value>,
    pub track: Option<NeteaseTrack>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeteaseTrackMeta {
    pub song_id: u64,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_ms: Option<i64>,
    pub level: String,
    pub stream_url: Option<String>,
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
pub struct SidecarCoverRef {
    pub kind: String,
    pub value: String,
    pub mime: Option<String>,
}

pub struct NeteaseSourceCatalogInstance {
    config: NeteaseSourceConfig,
}

impl ConfigUpdatable for NeteaseSourceCatalogInstance {
    fn apply_config_update_json(&mut self, new_config_json: &str) -> SdkResult<()> {
        self.config = serde_json::from_str(new_config_json).map_err(SdkError::from)?;
        Ok(())
    }
}

impl SourceCatalogDescriptor for NeteaseSourceCatalogInstance {
    type Config = NeteaseSourceConfig;
    type Instance = NeteaseSourceCatalogInstance;

    const TYPE_ID: &'static str = SOURCE_TYPE_ID;
    const DISPLAY_NAME: &'static str = SOURCE_DISPLAY_NAME;
    const CONFIG_SCHEMA_JSON: &'static str = CONFIG_SCHEMA_JSON;

    fn default_config() -> Self::Config {
        NeteaseSourceConfig::default()
    }

    fn create(config: Self::Config) -> SdkResult<Self::Instance> {
        Ok(NeteaseSourceCatalogInstance { config })
    }
}

#[stellatune_plugin_sdk::async_trait]
impl SourceCatalogInstance for NeteaseSourceCatalogInstance {
    type Stream = BytesSourceStream;

    async fn list_items_json(&mut self, request_json: &str) -> SdkResult<String> {
        let request: NeteaseListRequest =
            serde_json::from_str(request_json).map_err(SdkError::from)?;
        let items = list_items_async(&self.config, &request).await?;
        serde_json::to_string(&items).map_err(SdkError::from)
    }

    async fn open_stream_json(
        &mut self,
        track_json: &str,
    ) -> SdkResult<SourceOpenResult<Self::Stream>> {
        let track: NeteaseTrack = serde_json::from_str(track_json).map_err(SdkError::from)?;
        let (stream, track_meta) = open_stream_async(&self.config, &track).await?;
        let raw = serde_json::to_string(&track_meta).map_err(SdkError::from)?;
        Ok(SourceOpenResult::new(stream).with_track_meta_json(raw))
    }
}

async fn list_items_async(
    config: &NeteaseSourceConfig,
    request: &NeteaseListRequest,
) -> SdkResult<Vec<NeteaseListItem>> {
    if request.action.trim().eq_ignore_ascii_case("list_playlists") {
        return fetch_playlist_items(config, request).await;
    }

    let items = fetch_song_items(config, request).await?;
    let level = normalize_level(request.level.as_deref(), config);
    Ok(items
        .into_iter()
        .map(|item| {
            let song_id = item.song_id;
            let title = item
                .title
                .clone()
                .filter(|v| !v.trim().is_empty())
                .unwrap_or_else(|| format!("Song {song_id}"));
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

async fn open_stream_async(
    config: &NeteaseSourceConfig,
    track: &NeteaseTrack,
) -> SdkResult<(BytesSourceStream, NeteaseTrackMeta)> {
    ensure_sidecar_running(config).await?;
    let resolved = resolve_stream_url(config, track).await?;
    let bytes = download_audio_bytes(config, &resolved.url).await?;
    let ext_hint = guess_ext_hint(&resolved.url, resolved.ext_hint.as_deref())
        .or_else(|| track.ext_hint.clone())
        .unwrap_or_else(|| "mp3".to_string());

    host_log(
        StLogLevel::Debug,
        &format!(
            "netease open_stream song_id={} level={} ext_hint={}",
            track.song_id, track.level, ext_hint
        ),
    );

    let track_meta = NeteaseTrackMeta {
        song_id: track.song_id,
        title: track.title.clone(),
        artist: track.artist.clone(),
        album: track.album.clone(),
        duration_ms: track.duration_ms,
        level: track.level.clone(),
        stream_url: Some(resolved.url),
    };
    Ok((BytesSourceStream::new(bytes), track_meta))
}

pub struct BytesSourceStream {
    cursor: Cursor<Vec<u8>>,
}

impl BytesSourceStream {
    fn new(bytes: Vec<u8>) -> Self {
        Self {
            cursor: Cursor::new(bytes),
        }
    }
}

impl SourceStream for BytesSourceStream {
    const SUPPORTS_SEEK: bool = true;
    const SUPPORTS_TELL: bool = true;
    const SUPPORTS_SIZE: bool = true;

    fn read(&mut self, out: &mut [u8]) -> SdkResult<usize> {
        self.cursor.read(out).map_err(SdkError::from)
    }

    fn seek(&mut self, offset: i64, whence: StSeekWhence) -> SdkResult<u64> {
        let next = match whence {
            StSeekWhence::Start => {
                if offset < 0 {
                    return Err(SdkError::invalid_arg("seek before start"));
                }
                SeekFrom::Start(offset as u64)
            },
            StSeekWhence::Current => SeekFrom::Current(offset),
            StSeekWhence::End => SeekFrom::End(offset),
        };
        self.cursor.seek(next).map_err(SdkError::from)
    }

    fn tell(&mut self) -> SdkResult<u64> {
        Ok(self.cursor.position())
    }

    fn size(&mut self) -> SdkResult<u64> {
        Ok(self.cursor.get_ref().len() as u64)
    }
}

async fn fetch_song_items(
    config: &NeteaseSourceConfig,
    request: &NeteaseListRequest,
) -> SdkResult<Vec<SidecarSongItem>> {
    let limit = request.limit.clamp(1, 1000);
    let offset = request.offset;

    match request.action.trim().to_ascii_lowercase().as_str() {
        "ensure_sidecar" => {
            ensure_sidecar_running(config).await?;
            Ok(Vec::new())
        },
        "shutdown_sidecar" => {
            shutdown_sidecar(config).await?;
            Ok(Vec::new())
        },
        "playlist_tracks" => {
            let playlist_id = extract_playlist_id(request)
                .ok_or_else(|| SdkError::invalid_arg("playlist_id is required"))?;
            let params = vec![
                ("playlist_id", playlist_id.to_string()),
                ("limit", limit.to_string()),
                ("offset", offset.to_string()),
                ("level", normalize_level(request.level.as_deref(), config)),
            ];
            let response: SidecarSongListResponse =
                sidecar_get_json(config, "/v1/playlist/tracks", &params).await?;
            Ok(response.items)
        },
        _ => {
            let keywords = request.keywords.trim();
            if keywords.is_empty() {
                return Ok(Vec::new());
            }
            let params = vec![
                ("keywords", keywords.to_string()),
                ("limit", limit.to_string()),
                ("offset", offset.to_string()),
                ("level", normalize_level(request.level.as_deref(), config)),
            ];
            let response: SidecarSongListResponse =
                sidecar_get_json(config, "/v1/search", &params).await?;
            Ok(response.items)
        },
    }
}

async fn fetch_playlist_items(
    config: &NeteaseSourceConfig,
    request: &NeteaseListRequest,
) -> SdkResult<Vec<NeteaseListItem>> {
    ensure_sidecar_running(config).await?;
    let limit = request.limit.clamp(1, 200);
    let offset = request.offset;
    host_log(
        StLogLevel::Debug,
        &format!(
            "netease list_playlists begin limit={limit} offset={offset} base_url={}",
            normalize_base_url(&config.sidecar_base_url)
        ),
    );
    let params = vec![
        ("limit", limit.to_string()),
        ("offset", offset.to_string()),
        ("source_label", SOURCE_DISPLAY_NAME.to_string()),
    ];
    let response: SidecarPlaylistListResponse =
        sidecar_get_json(config, "/v1/playlists", &params).await?;
    let count = response.items.len();
    host_log(
        StLogLevel::Info,
        &format!("netease list_playlists fetched count={count}"),
    );
    Ok(response
        .items
        .into_iter()
        .map(|item| NeteaseListItem {
            kind: "playlist".to_string(),
            item_id: item.playlist_id.clone(),
            source_id: item.source_id.unwrap_or_else(|| "netease".to_string()),
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

async fn resolve_stream_url(
    config: &NeteaseSourceConfig,
    track: &NeteaseTrack,
) -> SdkResult<SidecarSongUrlResponse> {
    if let Some(url) = track.stream_url.as_deref()
        && !url.trim().is_empty()
    {
        return Ok(SidecarSongUrlResponse {
            url: url.to_string(),
            ext_hint: track.ext_hint.clone(),
        });
    }

    let params = vec![
        ("song_id", track.song_id.to_string()),
        ("level", normalize_level(Some(&track.level), config)),
    ];
    sidecar_get_json(config, "/v1/song/url", &params).await
}

async fn download_audio_bytes(
    config: &NeteaseSourceConfig,
    stream_url: &str,
) -> SdkResult<Vec<u8>> {
    let client = sidecar_client(config)?;
    let response = client
        .get(stream_url)
        .send()
        .await
        .map_err(|e| SdkError::msg(format!("download stream failed: {e}")))?;
    let response = response
        .error_for_status()
        .map_err(|e| SdkError::msg(format!("stream http status error: {e}")))?;
    let bytes = response
        .bytes()
        .await
        .map_err(|e| SdkError::msg(format!("stream read failed: {e}")))?;
    if bytes.is_empty() {
        return Err(SdkError::msg("empty stream payload"));
    }
    Ok(bytes.to_vec())
}

async fn sidecar_get_json<T: DeserializeOwned>(
    config: &NeteaseSourceConfig,
    path: &str,
    params: &[(&str, String)],
) -> SdkResult<T> {
    ensure_sidecar_running(config).await?;
    let base_url = normalize_base_url(&config.sidecar_base_url);
    let full_url = format!("{base_url}{path}");
    host_log(
        StLogLevel::Debug,
        &format!(
            "netease sidecar request GET path={path} params={}",
            format_query_params(params)
        ),
    );
    let client = sidecar_client(config)?;
    let response = client
        .get(&full_url)
        .query(params)
        .send()
        .await
        .map_err(|e| {
            host_log(
                StLogLevel::Warn,
                &format!("netease sidecar request failed path={path}: {e}"),
            );
            SdkError::msg(format!("sidecar request failed: {e}"))
        })?;
    let response = response.error_for_status().map_err(|e| {
        host_log(
            StLogLevel::Warn,
            &format!("netease sidecar status error path={path}: {e}"),
        );
        SdkError::msg(format!("sidecar status error: {e}"))
    })?;
    response.json::<T>().await.map_err(|e| {
        host_log(
            StLogLevel::Warn,
            &format!("netease sidecar json decode failed path={path}: {e}"),
        );
        SdkError::msg(format!("sidecar json decode failed: {e}"))
    })
}

fn sidecar_client(config: &NeteaseSourceConfig) -> SdkResult<reqwest::Client> {
    let timeout_ms = config.request_timeout_ms.max(500);
    sidecar_client_with_timeout(timeout_ms)
}

fn sidecar_health_client(config: &NeteaseSourceConfig) -> SdkResult<reqwest::Client> {
    let timeout_ms = config
        .request_timeout_ms
        .clamp(500, SIDECAR_HEALTH_TIMEOUT_MS);
    sidecar_client_with_timeout(timeout_ms)
}

fn sidecar_client_with_timeout(timeout_ms: u64) -> SdkResult<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_millis(timeout_ms))
        .build()
        .map_err(|e| {
            SdkError::msg(format!(
                "create sidecar client failed (timeout_ms={timeout_ms}): {e}"
            ))
        })
}

fn reset_sidecar_start_state_if_needed() {
    let Some(state_lock) = SIDECAR_START_STATE.get() else {
        return;
    };
    if let Ok(mut state) = state_lock.lock()
        && state.consecutive_failures != 0
    {
        state.mark_success();
    }
}

async fn shutdown_sidecar(config: &NeteaseSourceConfig) -> SdkResult<()> {
    let base_url = normalize_base_url(&config.sidecar_base_url);
    let full_url = format!("{base_url}/v1/admin/shutdown");
    let client = sidecar_client(config)?;
    match client.get(&full_url).send().await {
        Ok(response) => {
            if let Err(e) = response.error_for_status_ref() {
                host_log(
                    StLogLevel::Warn,
                    &format!("netease sidecar shutdown status error: {e}"),
                );
                return Err(SdkError::msg(format!("sidecar shutdown status error: {e}")));
            }
            host_log(StLogLevel::Info, "netease sidecar shutdown requested");
            Ok(())
        },
        Err(e) => {
            host_log(
                StLogLevel::Debug,
                &format!("netease sidecar shutdown skipped (unreachable): {e}"),
            );
            Ok(())
        },
    }
}

async fn ensure_sidecar_running(config: &NeteaseSourceConfig) -> SdkResult<()> {
    let base_url = normalize_base_url(&config.sidecar_base_url);
    host_log(
        StLogLevel::Debug,
        &format!(
            "netease ensure_sidecar_running base_url={base_url} sidecar_path={:?} args={:?}",
            config.sidecar_path, config.sidecar_args
        ),
    );

    match sidecar_health_result(config).await {
        Ok(true) => {
            reset_sidecar_start_state_if_needed();
            host_log(StLogLevel::Debug, "netease sidecar already healthy");
            return Ok(());
        },
        Ok(false) => {
            host_log(
                StLogLevel::Debug,
                "netease sidecar health returned ok=false",
            );
        },
        Err(err) => {
            host_log(
                StLogLevel::Debug,
                &format!("netease sidecar health check failed: {err}"),
            );
        },
    }

    let lock = SIDECAR_START_LOCK.get_or_init(|| tokio::sync::Mutex::new(()));
    let _guard = lock.lock().await;

    match sidecar_health_result(config).await {
        Ok(true) => {
            reset_sidecar_start_state_if_needed();
            host_log(
                StLogLevel::Debug,
                "netease sidecar became healthy while waiting start lock",
            );
            return Ok(());
        },
        Ok(false) => {},
        Err(err) => {
            host_log(
                StLogLevel::Debug,
                &format!("netease sidecar health recheck failed: {err}"),
            );
        },
    }

    let state_lock = SIDECAR_START_STATE.get_or_init(|| Mutex::new(SidecarStartState::default()));
    {
        let state = state_lock
            .lock()
            .map_err(|_| SdkError::msg("sidecar start state mutex poisoned"))?;
        if let Some(remaining) = state.remaining_cooldown() {
            let remaining_ms = remaining.as_millis();
            host_log(
                StLogLevel::Warn,
                &format!(
                    "netease sidecar start throttled consecutive_failures={} cooldown_remaining_ms={remaining_ms}",
                    state.consecutive_failures
                ),
            );
            return Err(SdkError::msg(format!(
                "sidecar start throttled after repeated failures; retry after {remaining_ms} ms"
            )));
        }
    }

    let start_result = match spawn_sidecar_process(config).await {
        Ok(()) => wait_sidecar_ready(config).await,
        Err(err) => Err(err),
    };
    let mut state = state_lock
        .lock()
        .map_err(|_| SdkError::msg("sidecar start state mutex poisoned"))?;
    match start_result {
        Ok(()) => {
            if state.consecutive_failures != 0 {
                host_log(
                    StLogLevel::Info,
                    &format!(
                        "netease sidecar recovered after {} failed starts",
                        state.consecutive_failures
                    ),
                );
            }
            state.mark_success();
            Ok(())
        },
        Err(err) => {
            state.mark_failure();
            let cooldown_ms = state.current_cooldown().as_millis();
            host_log(
                StLogLevel::Warn,
                &format!(
                    "netease sidecar start failed consecutive_failures={} next_retry_cooldown_ms={cooldown_ms}: {err}",
                    state.consecutive_failures
                ),
            );
            Err(err)
        },
    }
}

async fn wait_sidecar_ready(config: &NeteaseSourceConfig) -> SdkResult<()> {
    let deadline = Instant::now() + Duration::from_secs(SIDECAR_READY_TIMEOUT_SECS);
    let mut attempts: u32 = 0;
    while Instant::now() < deadline {
        attempts = attempts.saturating_add(1);
        match sidecar_health_result(config).await {
            Ok(true) => {
                host_log(
                    StLogLevel::Info,
                    &format!("netease sidecar ready after {attempts} health checks"),
                );
                return Ok(());
            },
            Ok(false) => {
                if attempts == 1 || attempts.is_multiple_of(8) {
                    host_log(
                        StLogLevel::Debug,
                        &format!("netease sidecar not ready yet (attempt={attempts})"),
                    );
                }
            },
            Err(err) => {
                if attempts == 1 || attempts.is_multiple_of(8) {
                    host_log(
                        StLogLevel::Debug,
                        &format!("netease sidecar health attempt={attempts} failed: {err}"),
                    );
                }
            },
        }
        sleep(Duration::from_millis(150)).await;
    }
    let base_url = normalize_base_url(&config.sidecar_base_url);
    host_log(
        StLogLevel::Error,
        &format!(
            "netease sidecar did not become ready in time base_url={base_url} attempts={attempts}"
        ),
    );
    Err(SdkError::msg("sidecar did not become ready in time"))
}

async fn sidecar_health_result(config: &NeteaseSourceConfig) -> Result<bool, String> {
    let client = sidecar_health_client(config).map_err(|e| e.to_string())?;
    let url = format!("{}/health", normalize_base_url(&config.sidecar_base_url));
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;
    let response = response
        .error_for_status()
        .map_err(|e| format!("status error: {e}"))?;
    response
        .json::<SidecarHealthResponse>()
        .await
        .map(|v| v.ok)
        .map_err(|e| format!("json decode failed: {e}"))
}

async fn spawn_sidecar_process(config: &NeteaseSourceConfig) -> SdkResult<()> {
    let mut cmd = build_sidecar_command(config)?;
    let owner_pid = std::process::id();
    cmd.args(&config.sidecar_args);
    cmd.env("STELLATUNE_NCM_OWNER_PID", owner_pid.to_string());
    cmd.stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    let mut child = cmd
        .spawn()
        .map_err(|e| SdkError::msg(format!("spawn sidecar failed: {e}")))?;
    let pid = child.id();
    host_log(
        StLogLevel::Info,
        &format!(
            "netease sidecar spawn requested pid={pid} owner_pid={owner_pid} sidecar_path={:?} args={:?}",
            config.sidecar_path, config.sidecar_args
        ),
    );

    sleep(Duration::from_millis(200)).await;
    match child.try_wait() {
        Ok(Some(status)) => {
            host_log(
                StLogLevel::Error,
                &format!("netease sidecar exited immediately pid={pid} status={status}"),
            );
            return Err(SdkError::msg(format!(
                "sidecar exited immediately after spawn (status={status})"
            )));
        },
        Ok(None) => {
            host_log(
                StLogLevel::Debug,
                &format!("netease sidecar process is running pid={pid}"),
            );
        },
        Err(err) => {
            host_log(
                StLogLevel::Warn,
                &format!("netease sidecar try_wait failed pid={pid}: {err}"),
            );
        },
    }
    Ok(())
}

fn build_sidecar_command(config: &NeteaseSourceConfig) -> SdkResult<Command> {
    if let Some(raw_path) = config.sidecar_path.as_deref() {
        let sidecar_path = raw_path.trim();
        if sidecar_path.is_empty() {
            return Err(SdkError::invalid_arg("sidecar_path is empty"));
        }
        if std::path::Path::new(sidecar_path).is_absolute() {
            host_log(
                StLogLevel::Info,
                &format!("netease sidecar uses explicit absolute path: {sidecar_path}"),
            );
            let mut cmd = Command::new(sidecar_path);

            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                const CREATE_NO_WINDOW: u32 = 0x08000000;
                if !cfg!(debug_assertions) {
                    cmd.creation_flags(CREATE_NO_WINDOW);
                }
            }

            if let Some(root) = resolve_runtime_path(".") {
                cmd.current_dir(root);
            }
            return Ok(cmd);
        }
        host_log(
            StLogLevel::Info,
            &format!("netease sidecar uses explicit relative path: {sidecar_path}"),
        );
        return sidecar_command(sidecar_path)
            .map_err(|e| SdkError::msg(format!("resolve sidecar command failed: {e}")));
    }

    let candidates = default_sidecar_candidates();
    let mut tried_paths = Vec::new();
    for candidate in candidates {
        if let Some(path) = resolve_runtime_path(candidate) {
            let exists = path.exists();
            tried_paths.push(format!(
                "{candidate} => {} (exists={exists})",
                path.display()
            ));
            if exists {
                host_log(
                    StLogLevel::Info,
                    &format!("netease sidecar selected candidate: {candidate}"),
                );
                return sidecar_command(candidate)
                    .map_err(|e| SdkError::msg(format!("build sidecar command failed: {e}")));
            }
        } else {
            tried_paths.push(format!("{candidate} => <unresolved>"));
        }
    }

    if let Some(runtime_root) = resolve_runtime_path(".") {
        host_log(
            StLogLevel::Error,
            &format!(
                "netease sidecar executable not found under runtime_root={} tried: {}",
                runtime_root.display(),
                tried_paths.join("; ")
            ),
        );
    } else {
        host_log(
            StLogLevel::Error,
            &format!(
                "netease sidecar executable not found (runtime_root unavailable) tried: {}",
                tried_paths.join("; ")
            ),
        );
    }

    Err(SdkError::msg(format!(
        "sidecar executable not found; tried {}",
        candidates.join(", ")
    )))
}

fn default_sidecar_candidates() -> &'static [&'static str] {
    if cfg!(windows) {
        &[
            "stellatune-ncm-sidecar.exe",
            "bin/stellatune-ncm-sidecar.exe",
            "bin/stellatune-ncm-sidecar.cmd",
        ]
    } else {
        &["stellatune-ncm-sidecar", "bin/stellatune-ncm-sidecar"]
    }
}

fn normalize_base_url(raw: &str) -> String {
    let value = raw.trim();
    if value.is_empty() {
        return DEFAULT_SIDECAR_BASE_URL.to_string();
    }
    value.trim_end_matches('/').to_string()
}

fn normalize_ext_hint(raw: Option<&str>) -> String {
    raw.map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| v.to_ascii_lowercase())
        .unwrap_or_else(|| "mp3".to_string())
}

fn normalize_level(raw: Option<&str>, config: &NeteaseSourceConfig) -> String {
    let level = raw
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToString::to_string)
        .or_else(|| {
            let cfg = config.default_level.trim();
            (!cfg.is_empty()).then(|| cfg.to_string())
        })
        .unwrap_or_else(|| DEFAULT_LEVEL.to_string());
    level.to_ascii_lowercase()
}

fn format_query_params(params: &[(&str, String)]) -> String {
    params
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&")
}

fn guess_ext_hint(stream_url: &str, fallback: Option<&str>) -> Option<String> {
    let parsed = Url::parse(stream_url).ok();
    if let Some(url) = parsed
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

    fallback
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| v.to_ascii_lowercase())
}
