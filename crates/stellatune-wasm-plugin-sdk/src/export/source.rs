#[macro_export]
macro_rules! export_source_component {
    (
        plugin_type: $plugin_ty:ty,
        create: $create:path $(,)?
    ) => {
        mod __st_source_component_export {
            use super::*;
            use $crate::__private::parking_lot::{Mutex, MutexGuard};
            use std::sync::OnceLock;
            use $crate::__private::stellatune_wasm_guest_bindings_source as __st_bindings;

            type __StPlugin = $plugin_ty;
            type __StPluginError =
                __st_bindings::exports::stellatune::plugin::source::PluginError;
            type __StDisableReason =
                __st_bindings::exports::stellatune::plugin::lifecycle::DisableReason;
            type __StConfigUpdateMode =
                __st_bindings::stellatune::plugin::common::ConfigUpdateMode;
            type __StConfigUpdatePlan =
                __st_bindings::exports::stellatune::plugin::source::ConfigUpdatePlan;
            type __StMediaMetadata = __st_bindings::exports::stellatune::plugin::source::MediaMetadata;
            type __StEncodedChunk = __st_bindings::exports::stellatune::plugin::source::EncodedChunk;
            type __StEncodedAudioFormat =
                __st_bindings::stellatune::plugin::common::EncodedAudioFormat;
            type __StAudioTags = __st_bindings::stellatune::plugin::common::AudioTags;
            type __StMetadataEntry = __st_bindings::stellatune::plugin::common::MetadataEntry;
            type __StMetadataValue = __st_bindings::stellatune::plugin::common::MetadataValue;

            static __ST_PLUGIN: OnceLock<Mutex<__StPlugin>> = OnceLock::new();

            struct __StRoot;
            struct __StCatalog {
                inner: Mutex<<__StPlugin as $crate::SourcePlugin>::Catalog>,
            }
            struct __StSourceStream {
                inner: Mutex<<<__StPlugin as $crate::SourcePlugin>::Catalog as $crate::SourceCatalog>::Stream>,
            }

            fn __map_error(error: $crate::SdkError) -> __StPluginError {
                match error {
                    $crate::SdkError::InvalidArg(message) => __StPluginError::InvalidArg(message),
                    $crate::SdkError::NotFound(message) => __StPluginError::NotFound(message),
                    $crate::SdkError::Io(message) => __StPluginError::Io(message),
                    $crate::SdkError::Timeout(message) => __StPluginError::Timeout(message),
                    $crate::SdkError::Unsupported(message) => __StPluginError::Unsupported(message),
                    $crate::SdkError::Denied(message) => __StPluginError::Denied(message),
                    $crate::SdkError::Internal(message) => __StPluginError::Internal(message),
                }
            }

            fn __map_disable_reason(reason: __StDisableReason) -> $crate::common::DisableReason {
                match reason {
                    __StDisableReason::HostDisable => $crate::common::DisableReason::HostDisable,
                    __StDisableReason::Unload => $crate::common::DisableReason::Unload,
                    __StDisableReason::Shutdown => $crate::common::DisableReason::Shutdown,
                    __StDisableReason::Reload => $crate::common::DisableReason::Reload,
                }
            }

            fn __map_config_update_mode(
                mode: $crate::common::ConfigUpdateMode,
            ) -> __StConfigUpdateMode {
                match mode {
                    $crate::common::ConfigUpdateMode::HotApply => __StConfigUpdateMode::HotApply,
                    $crate::common::ConfigUpdateMode::Recreate => __StConfigUpdateMode::Recreate,
                    $crate::common::ConfigUpdateMode::Reject => __StConfigUpdateMode::Reject,
                }
            }

            fn __map_config_update_plan(plan: $crate::common::ConfigUpdatePlan) -> __StConfigUpdatePlan {
                __StConfigUpdatePlan {
                    mode: __map_config_update_mode(plan.mode),
                    reason: plan.reason,
                }
            }

            fn __map_metadata_value(value: $crate::common::MetadataValue) -> __StMetadataValue {
                match value {
                    $crate::common::MetadataValue::Text(text) => __StMetadataValue::Text(text),
                    $crate::common::MetadataValue::Boolean(v) => __StMetadataValue::Boolean(v),
                    $crate::common::MetadataValue::Uint32(v) => __StMetadataValue::Uint32(v),
                    $crate::common::MetadataValue::Uint64(v) => __StMetadataValue::Uint64(v),
                    $crate::common::MetadataValue::Int64(v) => __StMetadataValue::Int64(v),
                    $crate::common::MetadataValue::Float64(v) => __StMetadataValue::Float64(v),
                    $crate::common::MetadataValue::Bytes(bytes) => __StMetadataValue::Bytes(bytes),
                }
            }

            fn __map_metadata_entry(entry: $crate::common::MetadataEntry) -> __StMetadataEntry {
                __StMetadataEntry {
                    key: entry.key,
                    value: __map_metadata_value(entry.value),
                }
            }

            fn __map_audio_tags(tags: $crate::common::AudioTags) -> __StAudioTags {
                __StAudioTags {
                    title: tags.title,
                    album: tags.album,
                    artists: tags.artists,
                    album_artists: tags.album_artists,
                    genres: tags.genres,
                    track_number: tags.track_number,
                    track_total: tags.track_total,
                    disc_number: tags.disc_number,
                    disc_total: tags.disc_total,
                    year: tags.year,
                    comment: tags.comment,
                }
            }

            fn __map_encoded_audio_format(
                format: $crate::common::EncodedAudioFormat,
            ) -> __StEncodedAudioFormat {
                __StEncodedAudioFormat {
                    codec: format.codec,
                    sample_rate: format.sample_rate,
                    channels: format.channels,
                    bitrate_kbps: format.bitrate_kbps,
                    container: format.container,
                }
            }

            fn __map_media_metadata(metadata: $crate::common::MediaMetadata) -> __StMediaMetadata {
                __StMediaMetadata {
                    tags: __map_audio_tags(metadata.tags),
                    duration_ms: metadata.duration_ms,
                    format: __map_encoded_audio_format(metadata.format),
                    extras: metadata.extras.into_iter().map(__map_metadata_entry).collect(),
                }
            }

            fn __map_encoded_chunk(chunk: $crate::common::EncodedChunk) -> __StEncodedChunk {
                __StEncodedChunk {
                    bytes: chunk.bytes,
                    eof: chunk.eof,
                }
            }

            fn __plugin_guard() -> Result<MutexGuard<'static, __StPlugin>, __StPluginError> {
                if __ST_PLUGIN.get().is_none() {
                    let plugin = ($create)().map_err(__map_error)?;
                    let _ = __ST_PLUGIN.set(Mutex::new(plugin));
                }
                let plugin = __ST_PLUGIN.get().ok_or_else(|| {
                    __StPluginError::Internal(
                        "plugin factory did not initialize global plugin state".to_string(),
                    )
                })?;
                Ok(plugin.lock())
            }

            impl __st_bindings::exports::stellatune::plugin::lifecycle::Guest for __StRoot {
                fn on_enable() -> Result<(), __StPluginError> {
                    let mut plugin = __plugin_guard()?;
                    plugin.on_enable().map_err(__map_error)
                }

                fn on_disable(reason: __StDisableReason) -> Result<(), __StPluginError> {
                    let mut plugin = __plugin_guard()?;
                    plugin
                        .on_disable(__map_disable_reason(reason))
                        .map_err(__map_error)
                }
            }

            impl __st_bindings::exports::stellatune::plugin::source::Guest for __StRoot {
                type SourceStream = __StSourceStream;
                type Catalog = __StCatalog;

                fn create(
                ) -> Result<__st_bindings::exports::stellatune::plugin::source::Catalog, __StPluginError>
                {
                    let mut plugin = __plugin_guard()?;
                    let catalog = plugin.create_catalog().map_err(__map_error)?;
                    Ok(__st_bindings::exports::stellatune::plugin::source::Catalog::new(
                        __StCatalog {
                            inner: Mutex::new(catalog),
                        },
                    ))
                }
            }

            impl __st_bindings::exports::stellatune::plugin::source::GuestSourceStream for __StSourceStream {
                fn metadata(&self) -> Result<__StMediaMetadata, __StPluginError> {
                    let stream = self.inner.lock();
                    stream
                        .metadata()
                        .map(__map_media_metadata)
                        .map_err(__map_error)
                }

                fn read(&self, max_bytes: u32) -> Result<__StEncodedChunk, __StPluginError> {
                    let mut stream = self.inner.lock();
                    stream.read(max_bytes).map(__map_encoded_chunk).map_err(__map_error)
                }

                fn close(&self) {
                    let mut stream = self.inner.lock();
                    let _ = stream.close();
                }
            }

            impl __st_bindings::exports::stellatune::plugin::source::GuestCatalog for __StCatalog {
                fn list_items_json(&self, request_json: String) -> Result<String, __StPluginError> {
                    let mut catalog = self.inner.lock();
                    catalog
                        .list_items_json(request_json.as_str())
                        .map_err(__map_error)
                }

                fn open_stream_json(
                    &self,
                    track_json: String,
                ) -> Result<__st_bindings::exports::stellatune::plugin::source::SourceStream, __StPluginError>
                {
                    let mut catalog = self.inner.lock();
                    let stream = catalog
                        .open_stream_json(track_json.as_str())
                        .map_err(__map_error)?;
                    Ok(__st_bindings::exports::stellatune::plugin::source::SourceStream::new(
                        __StSourceStream {
                            inner: Mutex::new(stream),
                        },
                    ))
                }

                fn open_uri(
                    &self,
                    uri: String,
                ) -> Result<__st_bindings::exports::stellatune::plugin::source::SourceStream, __StPluginError>
                {
                    let mut catalog = self.inner.lock();
                    let stream = catalog.open_uri(uri.as_str()).map_err(__map_error)?;
                    Ok(__st_bindings::exports::stellatune::plugin::source::SourceStream::new(
                        __StSourceStream {
                            inner: Mutex::new(stream),
                        },
                    ))
                }

                fn plan_config_update_json(
                    &self,
                    new_config_json: String,
                ) -> Result<__StConfigUpdatePlan, __StPluginError> {
                    let mut catalog = self.inner.lock();
                    catalog
                        .plan_config_update_json(new_config_json.as_str())
                        .map(__map_config_update_plan)
                        .map_err(__map_error)
                }

                fn apply_config_update_json(
                    &self,
                    new_config_json: String,
                ) -> Result<(), __StPluginError> {
                    let mut catalog = self.inner.lock();
                    catalog
                        .apply_config_update_json(new_config_json.as_str())
                        .map_err(__map_error)
                }

                fn export_state_json(&self) -> Result<Option<String>, __StPluginError> {
                    let catalog = self.inner.lock();
                    catalog.export_state_json().map_err(__map_error)
                }

                fn import_state_json(&self, state_json: String) -> Result<(), __StPluginError> {
                    let mut catalog = self.inner.lock();
                    catalog
                        .import_state_json(state_json.as_str())
                        .map_err(__map_error)
                }

                fn close(&self) {
                    let mut catalog = self.inner.lock();
                    let _ = catalog.close();
                }
            }

            __st_bindings::export!(__StRoot with_types_in __st_bindings);
        }
    };
}
