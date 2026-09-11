use std::path::Path;

use stellatune_audio_builtin_adapters::builtin_decoder::builtin_decoder_supported_extensions;
use stellatune_plugins::typescript::{TypeScriptRuntime, protocol::SourcePlanDto};

pub(crate) struct PluginMetadataProvider {
    runtime: std::sync::Arc<TypeScriptRuntime>,
    executor: tokio::runtime::Handle,
    client: reqwest::Client,
}

impl PluginMetadataProvider {
    pub(crate) fn new(runtime: std::sync::Arc<TypeScriptRuntime>) -> Self {
        Self {
            runtime,
            executor: tokio::runtime::Handle::current(),
            client: reqwest::Client::new(),
        }
    }
}

impl stellatune_library::metadata_provider::MetadataProvider for PluginMetadataProvider {
    fn supports(&self, path: &Path) -> bool {
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        !builtin_decoder_supported_extensions().contains(&extension)
            && self.runtime.local_file_extensions().contains(&extension)
    }

    fn inspect(
        &self,
        path: &Path,
    ) -> anyhow::Result<stellatune_library::metadata_provider::LocalFileMetadata> {
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        let (plugin, capability) =
            self.runtime
                .local_file_resolver(extension)?
                .ok_or_else(|| {
                    anyhow::anyhow!("local-source plugin for .{extension} is no longer enabled")
                })?;
        let path = std::fs::canonicalize(path)?;
        let result = self.executor.block_on(self.runtime.invoke(
            &plugin,
            &capability,
            None,
            "inspect-file",
            serde_json::json!({ "path": path }),
            None,
        ))?;
        let metadata: PluginFileMetadata = serde_json::from_value(result.value)?;
        let cover = if let Some(url) = metadata.cover_url {
            match self.executor.block_on(read_cover(&self.client, &url)) {
                Ok(bytes) => Some(bytes),
                Err(error) => {
                    tracing::warn!(path = %path.display(), %error, "plugin artwork fetch failed");
                    None
                },
            }
        } else {
            None
        };
        Ok(stellatune_library::metadata_provider::LocalFileMetadata {
            title: metadata.title,
            artist: metadata.artist,
            album: metadata.album,
            album_artist: metadata.album_artist,
            disc_number: metadata.disc_number,
            track_number: metadata.track_number,
            artists: metadata.artists,
            duration_ms: metadata.duration_ms,
            cover,
        })
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PluginFileMetadata {
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    album_artist: Option<String>,
    disc_number: Option<i64>,
    track_number: Option<i64>,
    #[serde(default)]
    artists: Vec<String>,
    duration_ms: Option<i64>,
    cover_url: Option<String>,
}

async fn read_cover(client: &reqwest::Client, url: &str) -> anyhow::Result<Vec<u8>> {
    // Match the library's embedded artwork limit, including chunked responses.
    const MAX_COVER_BYTES: usize = 12 * 1024 * 1024;
    let mut response = client
        .get(url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await?
        .error_for_status()?;
    anyhow::ensure!(
        response
            .content_length()
            .is_none_or(|size| size <= MAX_COVER_BYTES as u64),
        "plugin artwork exceeds 12 MiB"
    );
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        anyhow::ensure!(
            chunk.len() <= MAX_COVER_BYTES - bytes.len(),
            "plugin artwork exceeds 12 MiB"
        );
        bytes.extend_from_slice(&chunk);
    }
    anyhow::ensure!(!bytes.is_empty(), "plugin artwork is empty");
    Ok(bytes)
}

pub(crate) struct ResolvedLocalFile {
    pub source: crate::player_service::source::ResolvedSourceSpec,
    pub plugin_id: Option<String>,
    pub capability_id: Option<String>,
}

pub(crate) async fn resolve_local_file(
    runtime: &TypeScriptRuntime,
    path: &Path,
) -> Result<ResolvedLocalFile, String> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if extension.is_empty() || builtin_decoder_supported_extensions().contains(&extension) {
        return Ok(ResolvedLocalFile {
            source: crate::player_service::source::ResolvedSourceSpec::File {
                path: path.to_owned(),
                media: Default::default(),
            },
            plugin_id: None,
            capability_id: None,
        });
    }
    let (plugin_id, capability_id) = runtime.local_file_resolver(&extension).map_err(|error| error.to_string())?
        .ok_or_else(|| format!("no enabled local-source plugin supports .{extension}; install or enable a matching plugin"))?;
    let path = std::fs::canonicalize(path).map_err(|error| error.to_string())?;
    let result = runtime
        .invoke(
            &plugin_id,
            &capability_id,
            None,
            "resolve-file",
            serde_json::json!({ "path": path }),
            None,
        )
        .await
        .map_err(|error| error.to_string())?;
    let plan: SourcePlanDto =
        serde_json::from_value(result.value).map_err(|error| error.to_string())?;
    let source =
        super::typescript_source::source_plan_spec(plan).map_err(|error| error.to_string())?;
    Ok(ResolvedLocalFile {
        source,
        plugin_id: Some(plugin_id),
        capability_id: Some(capability_id),
    })
}
