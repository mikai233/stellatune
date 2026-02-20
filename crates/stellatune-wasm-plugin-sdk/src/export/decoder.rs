#[macro_export]
macro_rules! export_decoder_component {
    (
        plugin_type: $plugin_ty:ty,
        create: $create:path $(,)?
    ) => {
        mod __st_decoder_component_export {
            use super::*;
            use $crate::__private::parking_lot::{Mutex, MutexGuard};
            use std::sync::OnceLock;
            use $crate::__private::stellatune_wasm_guest_bindings_decoder as __st_bindings;

            type __StPlugin = $plugin_ty;
            type __StPluginError =
                __st_bindings::exports::stellatune::plugin::decoder::PluginError;
            type __StDisableReason =
                __st_bindings::exports::stellatune::plugin::lifecycle::DisableReason;
            type __StConfigUpdateMode =
                __st_bindings::stellatune::plugin::common::ConfigUpdateMode;
            type __StConfigUpdatePlan =
                __st_bindings::exports::stellatune::plugin::decoder::ConfigUpdatePlan;
            type __StSeekWhence = __st_bindings::stellatune::plugin::common::SeekWhence;
            type __StHostStreamHandle =
                __st_bindings::exports::stellatune::plugin::decoder::HostStreamHandle;
            type __StDecoderInfo = __st_bindings::exports::stellatune::plugin::decoder::DecoderInfo;
            type __StMediaMetadata =
                __st_bindings::exports::stellatune::plugin::decoder::MediaMetadata;
            type __StPcmF32Chunk = __st_bindings::exports::stellatune::plugin::decoder::PcmF32Chunk;
            type __StEncodedAudioFormat =
                __st_bindings::stellatune::plugin::common::EncodedAudioFormat;
            type __StAudioTags = __st_bindings::stellatune::plugin::common::AudioTags;
            type __StMetadataEntry = __st_bindings::stellatune::plugin::common::MetadataEntry;
            type __StMetadataValue = __st_bindings::stellatune::plugin::common::MetadataValue;

            static __ST_PLUGIN: OnceLock<Mutex<__StPlugin>> = OnceLock::new();

            struct __StRoot;
            struct __StSession {
                inner: Mutex<<__StPlugin as $crate::DecoderPlugin>::Session>,
            }

            struct __StDecoderInputStream {
                handle: __StHostStreamHandle,
            }

            impl Drop for __StDecoderInputStream {
                fn drop(&mut self) {
                    self.handle.close();
                }
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

            fn __map_host_stream_error(
                error: __st_bindings::stellatune::plugin::host_stream::PluginError,
            ) -> $crate::SdkError {
                match error {
                    __st_bindings::stellatune::plugin::host_stream::PluginError::InvalidArg(message) => {
                        $crate::SdkError::InvalidArg(message)
                    }
                    __st_bindings::stellatune::plugin::host_stream::PluginError::NotFound(message) => {
                        $crate::SdkError::NotFound(message)
                    }
                    __st_bindings::stellatune::plugin::host_stream::PluginError::Io(message) => {
                        $crate::SdkError::Io(message)
                    }
                    __st_bindings::stellatune::plugin::host_stream::PluginError::Timeout(message) => {
                        $crate::SdkError::Timeout(message)
                    }
                    __st_bindings::stellatune::plugin::host_stream::PluginError::Unsupported(message) => {
                        $crate::SdkError::Unsupported(message)
                    }
                    __st_bindings::stellatune::plugin::host_stream::PluginError::Denied(message) => {
                        $crate::SdkError::Denied(message)
                    }
                    __st_bindings::stellatune::plugin::host_stream::PluginError::Internal(message) => {
                        $crate::SdkError::Internal(message)
                    }
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

            fn __map_decoder_info(info: $crate::common::DecoderInfo) -> __StDecoderInfo {
                __StDecoderInfo {
                    sample_rate: info.sample_rate,
                    channels: info.channels,
                    duration_ms: info.duration_ms,
                    seekable: info.seekable,
                    encoder_delay_frames: info.encoder_delay_frames,
                    encoder_padding_frames: info.encoder_padding_frames,
                }
            }

            fn __map_pcm_f32_chunk(chunk: $crate::common::PcmF32Chunk) -> __StPcmF32Chunk {
                __StPcmF32Chunk {
                    interleaved_f32le: chunk.interleaved_f32le,
                    frames: chunk.frames,
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

            impl $crate::DecoderInputStream for __StDecoderInputStream {
                fn read(&mut self, max_bytes: u32) -> $crate::SdkResult<Vec<u8>> {
                    self.handle.read(max_bytes).map_err(__map_host_stream_error)
                }

                fn seek(&mut self, offset: i64, whence: $crate::common::SeekWhence) -> $crate::SdkResult<u64> {
                    let mapped = match whence {
                        $crate::common::SeekWhence::Start => __StSeekWhence::Start,
                        $crate::common::SeekWhence::Current => __StSeekWhence::Current,
                        $crate::common::SeekWhence::End => __StSeekWhence::End,
                    };
                    self.handle
                        .seek(offset, mapped)
                        .map_err(__map_host_stream_error)
                }

                fn tell(&mut self) -> $crate::SdkResult<u64> {
                    self.handle.tell().map_err(__map_host_stream_error)
                }

                fn size(&mut self) -> $crate::SdkResult<u64> {
                    self.handle.size().map_err(__map_host_stream_error)
                }
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

            impl __st_bindings::exports::stellatune::plugin::decoder::Guest for __StRoot {
                type Session = __StSession;

                fn open(input: __StHostStreamHandle, ext_hint: Option<String>) -> Result<__st_bindings::exports::stellatune::plugin::decoder::Session, __StPluginError>
                {
                    let mut plugin = __plugin_guard()?;
                    let mut stream = __StDecoderInputStream { handle: input };
                    let session = plugin
                        .open($crate::DecoderInput {
                            stream: &mut stream,
                            ext_hint: ext_hint.as_deref(),
                        })
                        .map_err(__map_error)?;
                    Ok(__st_bindings::exports::stellatune::plugin::decoder::Session::new(
                        __StSession {
                            inner: Mutex::new(session),
                        },
                    ))
                }
            }

            impl __st_bindings::exports::stellatune::plugin::decoder::GuestSession for __StSession {
                fn info(&self) -> Result<__StDecoderInfo, __StPluginError> {
                    let session = self.inner.lock();
                    session.info().map(__map_decoder_info).map_err(__map_error)
                }

                fn metadata(&self) -> Result<__StMediaMetadata, __StPluginError> {
                    let session = self.inner.lock();
                    session
                        .metadata()
                        .map(__map_media_metadata)
                        .map_err(__map_error)
                }

                fn read_pcm_f32(&self, max_frames: u32) -> Result<__StPcmF32Chunk, __StPluginError> {
                    let mut session = self.inner.lock();
                    session
                        .read_pcm_f32(max_frames)
                        .map(__map_pcm_f32_chunk)
                        .map_err(__map_error)
                }

                fn seek_ms(&self, position_ms: u64) -> Result<(), __StPluginError> {
                    let mut session = self.inner.lock();
                    session.seek_ms(position_ms).map_err(__map_error)
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
