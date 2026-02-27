#[macro_export]
macro_rules! export_encoder_component {
    (
        plugin_type: $plugin_ty:ty,
        create: $create:path $(,)?
    ) => {
        mod __st_encoder_component_export {
            use super::*;
            use $crate::__private::parking_lot::{Mutex, MutexGuard};
            use std::sync::OnceLock;
            use $crate::__private::stellatune_world_encoder as __st_bindings;

            type __StPlugin = $plugin_ty;
            type __StPluginError =
                __st_bindings::exports::stellatune::plugin::encoder::PluginError;
            type __StDisableReason =
                __st_bindings::exports::stellatune::plugin::lifecycle::DisableReason;
            type __StConfigUpdateMode =
                __st_bindings::stellatune::plugin::common::ConfigUpdateMode;
            type __StConfigUpdatePlan =
                __st_bindings::exports::stellatune::plugin::encoder::ConfigUpdatePlan;
            type __StAudioSpec = __st_bindings::exports::stellatune::plugin::encoder::AudioSpec;
            type __StEncodeTarget =
                __st_bindings::exports::stellatune::plugin::encoder::EncodeTarget;
            type __StMediaMetadata =
                __st_bindings::exports::stellatune::plugin::encoder::MediaMetadata;
            type __StEncodedAudioFormat =
                __st_bindings::exports::stellatune::plugin::encoder::EncodedAudioFormat;
            type __StEncodedChunk =
                __st_bindings::exports::stellatune::plugin::encoder::EncodedChunk;
            type __StPcmF32Chunk =
                __st_bindings::exports::stellatune::plugin::encoder::PcmF32Chunk;
            type __StAudioTags = __st_bindings::stellatune::plugin::common::AudioTags;
            type __StMetadataEntry = __st_bindings::stellatune::plugin::common::MetadataEntry;
            type __StMetadataValue = __st_bindings::stellatune::plugin::common::MetadataValue;

            static __ST_PLUGIN: OnceLock<Mutex<__StPlugin>> = OnceLock::new();

            struct __StRoot;
            struct __StSession {
                inner: Mutex<<__StPlugin as $crate::EncoderPlugin>::Session>,
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

            fn __map_metadata_value(value: __StMetadataValue) -> $crate::common::MetadataValue {
                match value {
                    __StMetadataValue::Text(text) => $crate::common::MetadataValue::Text(text),
                    __StMetadataValue::Boolean(v) => $crate::common::MetadataValue::Boolean(v),
                    __StMetadataValue::Uint32(v) => $crate::common::MetadataValue::Uint32(v),
                    __StMetadataValue::Uint64(v) => $crate::common::MetadataValue::Uint64(v),
                    __StMetadataValue::Int64(v) => $crate::common::MetadataValue::Int64(v),
                    __StMetadataValue::Float64(v) => $crate::common::MetadataValue::Float64(v),
                    __StMetadataValue::Bytes(bytes) => $crate::common::MetadataValue::Bytes(bytes),
                }
            }

            fn __map_metadata_entry(entry: __StMetadataEntry) -> $crate::common::MetadataEntry {
                $crate::common::MetadataEntry {
                    key: entry.key,
                    value: __map_metadata_value(entry.value),
                }
            }

            fn __map_audio_tags(tags: __StAudioTags) -> $crate::common::AudioTags {
                $crate::common::AudioTags {
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
                format: __StEncodedAudioFormat,
            ) -> $crate::common::EncodedAudioFormat {
                $crate::common::EncodedAudioFormat {
                    codec: format.codec,
                    sample_rate: format.sample_rate,
                    channels: format.channels,
                    bitrate_kbps: format.bitrate_kbps,
                    container: format.container,
                }
            }

            fn __map_media_metadata(metadata: __StMediaMetadata) -> $crate::common::MediaMetadata {
                $crate::common::MediaMetadata {
                    tags: __map_audio_tags(metadata.tags),
                    duration_ms: metadata.duration_ms,
                    format: __map_encoded_audio_format(metadata.format),
                    extras: metadata.extras.into_iter().map(__map_metadata_entry).collect(),
                }
            }

            fn __map_pcm_f32_chunk(chunk: __StPcmF32Chunk) -> $crate::common::PcmF32Chunk {
                $crate::common::PcmF32Chunk {
                    interleaved_f32le: chunk.interleaved_f32le,
                    frames: chunk.frames,
                    eof: chunk.eof,
                }
            }

            fn __map_audio_spec(spec: __StAudioSpec) -> $crate::common::AudioSpec {
                $crate::common::AudioSpec {
                    sample_rate: spec.sample_rate,
                    channels: spec.channels,
                }
            }

            fn __map_encode_target(target: __StEncodeTarget) -> $crate::EncodeTarget {
                $crate::EncodeTarget {
                    format: __map_encoded_audio_format(target.format),
                    ext_hint: target.ext_hint,
                    options_json: target.options_json,
                }
            }

            fn __into_audio_spec(spec: $crate::common::AudioSpec) -> __StAudioSpec {
                __StAudioSpec {
                    sample_rate: spec.sample_rate,
                    channels: spec.channels,
                }
            }

            fn __into_encoded_audio_format(
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

            fn __into_encoded_chunk(chunk: $crate::common::EncodedChunk) -> __StEncodedChunk {
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

            impl __st_bindings::exports::stellatune::plugin::encoder::Guest for __StRoot {
                type Session = __StSession;

                fn create(
                    input: __StAudioSpec,
                    target: __StEncodeTarget,
                    metadata: Option<__StMediaMetadata>,
                ) -> Result<__st_bindings::exports::stellatune::plugin::encoder::Session, __StPluginError>
                {
                    let mut plugin = __plugin_guard()?;
                    let session = plugin
                        .create_session(
                            __map_audio_spec(input),
                            __map_encode_target(target),
                            metadata.map(__map_media_metadata),
                        )
                        .map_err(__map_error)?;
                    Ok(__st_bindings::exports::stellatune::plugin::encoder::Session::new(
                        __StSession {
                            inner: Mutex::new(session),
                        },
                    ))
                }
            }

            impl __st_bindings::exports::stellatune::plugin::encoder::GuestSession for __StSession {
                fn input_spec(&self) -> __StAudioSpec {
                    let session = self.inner.lock();
                    __into_audio_spec(session.input_spec())
                }

                fn output_format(&self) -> Result<__StEncodedAudioFormat, __StPluginError> {
                    let session = self.inner.lock();
                    session
                        .output_format()
                        .map(__into_encoded_audio_format)
                        .map_err(__map_error)
                }

                fn write_pcm_f32(&self, chunk: __StPcmF32Chunk) -> Result<u32, __StPluginError> {
                    let mut session = self.inner.lock();
                    session
                        .write_pcm_f32(__map_pcm_f32_chunk(chunk))
                        .map_err(__map_error)
                }

                fn read_encoded(&self, max_bytes: u32) -> Result<__StEncodedChunk, __StPluginError> {
                    let mut session = self.inner.lock();
                    session
                        .read_encoded(max_bytes)
                        .map(__into_encoded_chunk)
                        .map_err(__map_error)
                }

                fn plan_config_update_json(
                    &self,
                    new_config_json: String,
                ) -> Result<__StConfigUpdatePlan, __StPluginError> {
                    let mut session = self.inner.lock();
                    session
                        .plan_config_update_json(new_config_json.as_str())
                        .map(__map_config_update_plan)
                        .map_err(__map_error)
                }

                fn apply_config_update_json(
                    &self,
                    new_config_json: String,
                ) -> Result<(), __StPluginError> {
                    let mut session = self.inner.lock();
                    session
                        .apply_config_update_json(new_config_json.as_str())
                        .map_err(__map_error)
                }

                fn export_state_json(&self) -> Result<Option<String>, __StPluginError> {
                    let session = self.inner.lock();
                    session.export_state_json().map_err(__map_error)
                }

                fn import_state_json(&self, state_json: String) -> Result<(), __StPluginError> {
                    let mut session = self.inner.lock();
                    session
                        .import_state_json(state_json.as_str())
                        .map_err(__map_error)
                }

                fn close(&self) {
                    let mut session = self.inner.lock();
                    let _ = session.close();
                }
            }

            __st_bindings::export!(__StRoot with_types_in __st_bindings);
        }
    };
}
