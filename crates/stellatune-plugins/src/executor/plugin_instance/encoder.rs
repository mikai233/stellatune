use std::collections::BTreeMap;
use std::sync::mpsc;

use crate::error::Result;
use wasmtime::Store;

use stellatune_host_bindings::generated as host_bindings;

use host_bindings::encoder_plugin::EncoderPlugin as EncoderBinding;
use host_bindings::encoder_plugin::exports::stellatune::plugin::encoder as encoder_exports;
use host_bindings::encoder_plugin::stellatune::plugin::common as encoder_common;

use crate::executor::plugin_cell::{PluginCell, PluginCellState};
use crate::executor::stores::encoder::EncoderStoreData;
use crate::executor::{
    WasmPluginController, WasmtimePluginController, WorldKind, call_encoder_on_disable,
    call_encoder_on_enable, classify_world, map_disable_reason_encoder,
};
use crate::manifest::AbilityKind;
use crate::runtime::model::{
    PluginDisableReason, RuntimeArtwork, RuntimeArtworkKind, RuntimeAudioSpec,
    RuntimeCapabilityDescriptor, RuntimeConfigUpdateMode, RuntimeConfigUpdatePlan,
    RuntimeEncodeTarget, RuntimeEncodedAudioFormat, RuntimeEncodedChunk,
    RuntimeEncoderSessionHandle, RuntimeMediaMetadata, RuntimeMetadataValue, RuntimePcmF32Chunk,
    RuntimePluginDirective, RuntimePluginInfo,
};

use crate::executor::plugin_instance::common::{map_decoder_plugin_error, reconcile_with};

pub trait EncoderPluginApi {
    fn create(
        &mut self,
        input: RuntimeAudioSpec,
        target: RuntimeEncodeTarget,
        metadata: Option<RuntimeMediaMetadata>,
    ) -> Result<RuntimeEncoderSessionHandle>;
    fn input_spec(&mut self, session: RuntimeEncoderSessionHandle) -> Result<RuntimeAudioSpec>;
    fn output_format(
        &mut self,
        session: RuntimeEncoderSessionHandle,
    ) -> Result<RuntimeEncodedAudioFormat>;
    fn write_pcm_f32(
        &mut self,
        session: RuntimeEncoderSessionHandle,
        chunk: RuntimePcmF32Chunk,
    ) -> Result<u32>;
    fn read_encoded(
        &mut self,
        session: RuntimeEncoderSessionHandle,
        max_bytes: u32,
    ) -> Result<RuntimeEncodedChunk>;
    fn plan_config_update_json(
        &mut self,
        session: RuntimeEncoderSessionHandle,
        config_json: &str,
    ) -> Result<RuntimeConfigUpdatePlan>;
    fn apply_config_update_json(
        &mut self,
        session: RuntimeEncoderSessionHandle,
        config_json: &str,
    ) -> Result<()>;
    fn export_state_json(&mut self, session: RuntimeEncoderSessionHandle)
    -> Result<Option<String>>;
    fn import_state_json(
        &mut self,
        session: RuntimeEncoderSessionHandle,
        state_json: &str,
    ) -> Result<()>;
    fn close(&mut self, session: RuntimeEncoderSessionHandle) -> Result<()>;
}

pub struct WasmtimeEncoderPlugin {
    plugin_id: String,
    component: PluginCell<Store<EncoderStoreData>, EncoderBinding>,
    next_session_handle: u64,
    sessions: BTreeMap<u64, wasmtime::component::ResourceAny>,
}

impl WasmtimeEncoderPlugin {
    pub fn plugin_id(&self) -> &str {
        &self.plugin_id
    }

    fn encoder_api(&self) -> encoder_exports::Guest {
        self.component.plugin.stellatune_plugin_encoder().clone()
    }

    fn alloc_session_handle(&mut self) -> u64 {
        let handle = if self.next_session_handle == 0 {
            1
        } else {
            self.next_session_handle
        };
        self.next_session_handle = handle.saturating_add(1);
        if self.next_session_handle == 0 {
            self.next_session_handle = 1;
        }
        handle
    }

    fn runtime_audio_spec_from(spec: encoder_exports::AudioSpec) -> RuntimeAudioSpec {
        RuntimeAudioSpec {
            sample_rate: spec.sample_rate,
            channels: spec.channels,
        }
    }

    fn runtime_encoded_audio_format_from(
        format: encoder_exports::EncodedAudioFormat,
    ) -> RuntimeEncodedAudioFormat {
        RuntimeEncodedAudioFormat {
            codec: format.codec,
            sample_rate: format.sample_rate,
            channels: format.channels,
            bitrate_kbps: format.bitrate_kbps,
            container: format.container,
        }
    }

    fn runtime_config_update_plan_from(
        plan: encoder_exports::ConfigUpdatePlan,
    ) -> RuntimeConfigUpdatePlan {
        RuntimeConfigUpdatePlan {
            mode: match plan.mode {
                encoder_common::ConfigUpdateMode::HotApply => RuntimeConfigUpdateMode::HotApply,
                encoder_common::ConfigUpdateMode::Recreate => RuntimeConfigUpdateMode::Recreate,
                encoder_common::ConfigUpdateMode::Reject => RuntimeConfigUpdateMode::Reject,
            },
            reason: plan.reason,
        }
    }

    fn encoded_audio_format_into(
        format: RuntimeEncodedAudioFormat,
    ) -> encoder_common::EncodedAudioFormat {
        encoder_common::EncodedAudioFormat {
            codec: format.codec,
            sample_rate: format.sample_rate,
            channels: format.channels,
            bitrate_kbps: format.bitrate_kbps,
            container: format.container,
        }
    }

    fn metadata_value_into(value: RuntimeMetadataValue) -> encoder_common::MetadataValue {
        match value {
            RuntimeMetadataValue::Text(value) => encoder_common::MetadataValue::Text(value),
            RuntimeMetadataValue::Boolean(value) => encoder_common::MetadataValue::Boolean(value),
            RuntimeMetadataValue::Uint32(value) => encoder_common::MetadataValue::Uint32(value),
            RuntimeMetadataValue::Uint64(value) => encoder_common::MetadataValue::Uint64(value),
            RuntimeMetadataValue::Int64(value) => encoder_common::MetadataValue::Int64(value),
            RuntimeMetadataValue::Float64(value) => encoder_common::MetadataValue::Float64(value),
            RuntimeMetadataValue::Bytes(value) => encoder_common::MetadataValue::Bytes(value),
        }
    }

    fn media_metadata_into(metadata: RuntimeMediaMetadata) -> encoder_common::MediaMetadata {
        encoder_common::MediaMetadata {
            tags: encoder_common::AudioTags {
                title: metadata.tags.title,
                album: metadata.tags.album,
                artists: metadata.tags.artists,
                album_artists: metadata.tags.album_artists,
                genres: metadata.tags.genres,
                track_number: metadata.tags.track_number,
                track_total: metadata.tags.track_total,
                disc_number: metadata.tags.disc_number,
                disc_total: metadata.tags.disc_total,
                year: metadata.tags.year,
                comment: metadata.tags.comment,
            },
            duration_ms: metadata.duration_ms,
            format: Self::encoded_audio_format_into(metadata.format),
            artworks: metadata
                .artworks
                .into_iter()
                .map(Self::artwork_into)
                .collect::<Vec<_>>(),
            extras: metadata
                .extras
                .into_iter()
                .map(|entry| encoder_common::MetadataEntry {
                    key: entry.key,
                    value: Self::metadata_value_into(entry.value),
                })
                .collect::<Vec<_>>(),
        }
    }

    fn artwork_kind_into(kind: RuntimeArtworkKind) -> encoder_common::ArtworkKind {
        match kind {
            RuntimeArtworkKind::FrontCover => encoder_common::ArtworkKind::FrontCover,
            RuntimeArtworkKind::BackCover => encoder_common::ArtworkKind::BackCover,
            RuntimeArtworkKind::Leaflet => encoder_common::ArtworkKind::Leaflet,
            RuntimeArtworkKind::Media => encoder_common::ArtworkKind::Media,
            RuntimeArtworkKind::Artist => encoder_common::ArtworkKind::Artist,
            RuntimeArtworkKind::Other => encoder_common::ArtworkKind::Other,
        }
    }

    fn artwork_into(artwork: RuntimeArtwork) -> encoder_common::Artwork {
        encoder_common::Artwork {
            kind: Self::artwork_kind_into(artwork.kind),
            mime: artwork.mime,
            description: artwork.description,
            width: artwork.width,
            height: artwork.height,
            data: artwork.data,
        }
    }

    fn reconcile_runtime(&mut self) -> Result<()> {
        let session_refs = self.sessions.values().cloned().collect::<Vec<_>>();
        let mut rebuilt = false;
        let mut destroyed = false;
        reconcile_with(
            &mut self.component,
            |store, plugin, config_json| {
                let encoder = plugin.stellatune_plugin_encoder();
                for session in &session_refs {
                    let plan = map_decoder_plugin_error(
                        encoder.session().call_plan_config_update_json(
                            &mut *store,
                            *session,
                            config_json,
                        )?,
                        "encoder.session.plan-config-update-json",
                    )?;
                    match plan.mode {
                        encoder_common::ConfigUpdateMode::HotApply => {
                            map_decoder_plugin_error(
                                encoder.session().call_apply_config_update_json(
                                    &mut *store,
                                    *session,
                                    config_json,
                                )?,
                                "encoder.session.apply-config-update-json",
                            )?;
                        },
                        encoder_common::ConfigUpdateMode::Recreate => {
                            return Err(crate::op_error!(
                                "encoder session requested recreate for config update"
                            ));
                        },
                        encoder_common::ConfigUpdateMode::Reject => {
                            return Err(crate::op_error!(
                                "encoder session rejected config update: {}",
                                plan.reason.unwrap_or_else(|| "unknown".to_string())
                            ));
                        },
                    }
                }
                Ok(())
            },
            |store, plugin| {
                let encoder = plugin.stellatune_plugin_encoder();
                for session in &session_refs {
                    let _ = encoder.session().call_close(&mut *store, *session);
                    let _ = (*session).resource_drop(&mut *store);
                }
                call_encoder_on_disable(
                    plugin,
                    store,
                    map_disable_reason_encoder(PluginDisableReason::Reload),
                )?;
                call_encoder_on_enable(plugin, store)?;
                rebuilt = true;
                Ok(())
            },
            |store, plugin, reason| {
                let encoder = plugin.stellatune_plugin_encoder();
                for session in &session_refs {
                    let _ = encoder.session().call_close(&mut *store, *session);
                    let _ = (*session).resource_drop(&mut *store);
                }
                call_encoder_on_disable(plugin, store, map_disable_reason_encoder(reason))?;
                destroyed = true;
                Ok(())
            },
        )?;
        if rebuilt || destroyed {
            self.sessions.clear();
        }
        Ok(())
    }
}

impl EncoderPluginApi for WasmtimeEncoderPlugin {
    fn create(
        &mut self,
        input: RuntimeAudioSpec,
        target: RuntimeEncodeTarget,
        metadata: Option<RuntimeMediaMetadata>,
    ) -> Result<RuntimeEncoderSessionHandle> {
        self.reconcile_runtime()?;
        let encoder = self.encoder_api();
        let create_target = encoder_exports::EncodeTarget {
            format: Self::encoded_audio_format_into(target.format),
            ext_hint: target.ext_hint,
            options_json: target.options_json,
        };
        let create_metadata = metadata.map(Self::media_metadata_into);
        let session = map_decoder_plugin_error(
            encoder.call_create(
                &mut self.component.store,
                encoder_exports::AudioSpec {
                    sample_rate: input.sample_rate,
                    channels: input.channels,
                },
                &create_target,
                create_metadata.as_ref(),
            )?,
            "encoder.create",
        )?;
        let handle = self.alloc_session_handle();
        self.sessions.insert(handle, session);
        Ok(RuntimeEncoderSessionHandle(handle))
    }

    fn input_spec(&mut self, session: RuntimeEncoderSessionHandle) -> Result<RuntimeAudioSpec> {
        let Some(session_ref) = self.sessions.get(&session.0).cloned() else {
            return Err(crate::op_error!(
                "encoder session `{}` not found",
                session.0
            ));
        };
        self.reconcile_runtime()?;
        let encoder = self.encoder_api();
        let spec = encoder
            .session()
            .call_input_spec(&mut self.component.store, session_ref)?;
        Ok(Self::runtime_audio_spec_from(spec))
    }

    fn output_format(
        &mut self,
        session: RuntimeEncoderSessionHandle,
    ) -> Result<RuntimeEncodedAudioFormat> {
        let Some(session_ref) = self.sessions.get(&session.0).cloned() else {
            return Err(crate::op_error!(
                "encoder session `{}` not found",
                session.0
            ));
        };
        self.reconcile_runtime()?;
        let encoder = self.encoder_api();
        let format = map_decoder_plugin_error(
            encoder
                .session()
                .call_output_format(&mut self.component.store, session_ref)?,
            "encoder.session.output-format",
        )?;
        Ok(Self::runtime_encoded_audio_format_from(format))
    }

    fn write_pcm_f32(
        &mut self,
        session: RuntimeEncoderSessionHandle,
        chunk: RuntimePcmF32Chunk,
    ) -> Result<u32> {
        let Some(session_ref) = self.sessions.get(&session.0).cloned() else {
            return Err(crate::op_error!(
                "encoder session `{}` not found",
                session.0
            ));
        };
        self.reconcile_runtime()?;
        let encoder = self.encoder_api();
        map_decoder_plugin_error(
            encoder.session().call_write_pcm_f32(
                &mut self.component.store,
                session_ref,
                &encoder_exports::PcmF32Chunk {
                    interleaved_f32le: chunk.interleaved_f32le,
                    frames: chunk.frames,
                    eof: chunk.eof,
                },
            )?,
            "encoder.session.write-pcm-f32",
        )
    }

    fn read_encoded(
        &mut self,
        session: RuntimeEncoderSessionHandle,
        max_bytes: u32,
    ) -> Result<RuntimeEncodedChunk> {
        let Some(session_ref) = self.sessions.get(&session.0).cloned() else {
            return Err(crate::op_error!(
                "encoder session `{}` not found",
                session.0
            ));
        };
        self.reconcile_runtime()?;
        let encoder = self.encoder_api();
        let chunk = map_decoder_plugin_error(
            encoder.session().call_read_encoded(
                &mut self.component.store,
                session_ref,
                max_bytes,
            )?,
            "encoder.session.read-encoded",
        )?;
        Ok(RuntimeEncodedChunk {
            bytes: chunk.bytes,
            eof: chunk.eof,
        })
    }

    fn plan_config_update_json(
        &mut self,
        session: RuntimeEncoderSessionHandle,
        config_json: &str,
    ) -> Result<RuntimeConfigUpdatePlan> {
        let Some(session_ref) = self.sessions.get(&session.0).cloned() else {
            return Err(crate::op_error!(
                "encoder session `{}` not found",
                session.0
            ));
        };
        self.reconcile_runtime()?;
        let encoder = self.encoder_api();
        let plan = map_decoder_plugin_error(
            encoder.session().call_plan_config_update_json(
                &mut self.component.store,
                session_ref,
                config_json,
            )?,
            "encoder.session.plan-config-update-json",
        )?;
        Ok(Self::runtime_config_update_plan_from(plan))
    }

    fn apply_config_update_json(
        &mut self,
        session: RuntimeEncoderSessionHandle,
        config_json: &str,
    ) -> Result<()> {
        let Some(session_ref) = self.sessions.get(&session.0).cloned() else {
            return Err(crate::op_error!(
                "encoder session `{}` not found",
                session.0
            ));
        };
        self.reconcile_runtime()?;
        let encoder = self.encoder_api();
        map_decoder_plugin_error(
            encoder.session().call_apply_config_update_json(
                &mut self.component.store,
                session_ref,
                config_json,
            )?,
            "encoder.session.apply-config-update-json",
        )?;
        Ok(())
    }

    fn export_state_json(
        &mut self,
        session: RuntimeEncoderSessionHandle,
    ) -> Result<Option<String>> {
        let Some(session_ref) = self.sessions.get(&session.0).cloned() else {
            return Err(crate::op_error!(
                "encoder session `{}` not found",
                session.0
            ));
        };
        self.reconcile_runtime()?;
        let encoder = self.encoder_api();
        map_decoder_plugin_error(
            encoder
                .session()
                .call_export_state_json(&mut self.component.store, session_ref)?,
            "encoder.session.export-state-json",
        )
    }

    fn import_state_json(
        &mut self,
        session: RuntimeEncoderSessionHandle,
        state_json: &str,
    ) -> Result<()> {
        let Some(session_ref) = self.sessions.get(&session.0).cloned() else {
            return Err(crate::op_error!(
                "encoder session `{}` not found",
                session.0
            ));
        };
        self.reconcile_runtime()?;
        let encoder = self.encoder_api();
        map_decoder_plugin_error(
            encoder.session().call_import_state_json(
                &mut self.component.store,
                session_ref,
                state_json,
            )?,
            "encoder.session.import-state-json",
        )?;
        Ok(())
    }

    fn close(&mut self, session: RuntimeEncoderSessionHandle) -> Result<()> {
        let Some(session_ref) = self.sessions.remove(&session.0) else {
            return Ok(());
        };
        let encoder = self.encoder_api();
        let _ = encoder
            .session()
            .call_close(&mut self.component.store, session_ref);
        let _ = session_ref.resource_drop(&mut self.component.store);
        Ok(())
    }
}

impl Drop for WasmtimeEncoderPlugin {
    fn drop(&mut self) {
        let sessions = std::mem::take(&mut self.sessions);
        let encoder = self.encoder_api();
        for (_, session_ref) in sessions {
            let _ = encoder
                .session()
                .call_close(&mut self.component.store, session_ref);
            let _ = session_ref.resource_drop(&mut self.component.store);
        }
        if self.component.state() != PluginCellState::Destroyed {
            let _ = call_encoder_on_disable(
                &self.component.plugin,
                &mut self.component.store,
                map_disable_reason_encoder(PluginDisableReason::HostDisable),
            );
        }
    }
}

impl WasmtimePluginController {
    pub fn create_encoder_plugin(
        &self,
        plugin_id: &str,
        type_id: &str,
    ) -> Result<WasmtimeEncoderPlugin> {
        let (plugin, capability) =
            self.resolve_capability(plugin_id, AbilityKind::Encoder, type_id)?;
        let plugin_id = plugin.id.trim();
        self.ensure_plugin_active(plugin_id)?;

        let component_path = plugin.root_dir.join(&capability.component_rel_path);
        let component = self
            .load_component_cached(&component_path)
            .map_err(|error| {
                crate::op_error!(
                    "failed to load component for plugin `{}` component `{}`: {error:#}",
                    plugin_id,
                    capability.component_id
                )
            })?;

        let (tx, rx) = mpsc::channel::<RuntimePluginDirective>();
        let component = match classify_world(&capability.world) {
            WorldKind::Encoder => {
                self.instantiate_encoder_component(plugin_id, &plugin.root_dir, &component, rx)?
            },
            _ => {
                return Err(crate::op_error!(
                    "capability world `{}` is not an encoder world",
                    capability.world
                ));
            },
        };

        self.register_directive_sender(plugin_id, tx)?;

        Ok(WasmtimeEncoderPlugin {
            plugin_id: plugin_id.to_string(),
            component,
            next_session_handle: 1,
            sessions: BTreeMap::new(),
        })
    }

    pub fn install_and_create_encoder_plugin(
        &self,
        plugin: &RuntimePluginInfo,
        capabilities: &[RuntimeCapabilityDescriptor],
        type_id: &str,
    ) -> Result<WasmtimeEncoderPlugin> {
        WasmPluginController::install_plugin(self, plugin, capabilities)?;
        self.create_encoder_plugin(&plugin.id, type_id)
    }
}
