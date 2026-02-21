use std::collections::BTreeMap;
use std::sync::mpsc;

use crate::error::Result;
use crate::host::stream::HostStreamHandle;
use wasmtime::Store;
use wasmtime::component::Resource;

use stellatune_host_bindings::generated as host_bindings;

use host_bindings::decoder_plugin::DecoderPlugin as DecoderBinding;
use host_bindings::decoder_plugin::stellatune::plugin::common as decoder_common;

use crate::executor::plugin_cell::{PluginCell, PluginCellState};
use crate::executor::stores::decoder::DecoderStoreData;
use crate::executor::{
    WasmPluginController, WasmtimePluginController, WorldKind, classify_world,
    map_disable_reason_decoder,
};
use crate::manifest::AbilityKind;
use crate::runtime::model::{
    PluginDisableReason, RuntimeAudioTags, RuntimeCapabilityDescriptor, RuntimeDecoderInfo,
    RuntimeDecoderSessionHandle, RuntimeEncodedAudioFormat, RuntimeMediaMetadata,
    RuntimeMetadataEntry, RuntimeMetadataValue, RuntimePcmF32Chunk, RuntimePluginDirective,
    RuntimePluginInfo,
};

use crate::executor::plugin_instance::common::{map_decoder_plugin_error, reconcile_with};

macro_rules! runtime_decoder_info_from {
    ($info:expr) => {{
        let info = $info;
        RuntimeDecoderInfo {
            sample_rate: info.sample_rate,
            channels: info.channels,
            duration_ms: info.duration_ms,
            seekable: info.seekable,
            encoder_delay_frames: info.encoder_delay_frames,
            encoder_padding_frames: info.encoder_padding_frames,
        }
    }};
}

macro_rules! runtime_pcm_chunk_from {
    ($chunk:expr) => {{
        let chunk = $chunk;
        RuntimePcmF32Chunk {
            interleaved_f32le: chunk.interleaved_f32le,
            frames: chunk.frames,
            eof: chunk.eof,
        }
    }};
}

macro_rules! runtime_media_metadata_from_decoder {
    ($meta:expr) => {{
        let meta = $meta;
        RuntimeMediaMetadata {
            tags: RuntimeAudioTags {
                title: meta.tags.title,
                album: meta.tags.album,
                artists: meta.tags.artists,
                album_artists: meta.tags.album_artists,
                genres: meta.tags.genres,
                track_number: meta.tags.track_number,
                track_total: meta.tags.track_total,
                disc_number: meta.tags.disc_number,
                disc_total: meta.tags.disc_total,
                year: meta.tags.year,
                comment: meta.tags.comment,
            },
            duration_ms: meta.duration_ms,
            format: RuntimeEncodedAudioFormat {
                codec: meta.format.codec,
                sample_rate: meta.format.sample_rate,
                channels: meta.format.channels,
                bitrate_kbps: meta.format.bitrate_kbps,
                container: meta.format.container,
            },
            extras: meta
                .extras
                .into_iter()
                .map(|entry| RuntimeMetadataEntry {
                    key: entry.key,
                    value: match entry.value {
                        decoder_common::MetadataValue::Text(v) => RuntimeMetadataValue::Text(v),
                        decoder_common::MetadataValue::Boolean(v) => {
                            RuntimeMetadataValue::Boolean(v)
                        },
                        decoder_common::MetadataValue::Uint32(v) => RuntimeMetadataValue::Uint32(v),
                        decoder_common::MetadataValue::Uint64(v) => RuntimeMetadataValue::Uint64(v),
                        decoder_common::MetadataValue::Int64(v) => RuntimeMetadataValue::Int64(v),
                        decoder_common::MetadataValue::Float64(v) => {
                            RuntimeMetadataValue::Float64(v)
                        },
                        decoder_common::MetadataValue::Bytes(v) => RuntimeMetadataValue::Bytes(v),
                    },
                })
                .collect::<Vec<_>>(),
        }
    }};
}

pub trait DecoderPluginApi {
    fn open_stream(
        &mut self,
        stream: Box<dyn HostStreamHandle>,
        ext_hint: Option<&str>,
    ) -> Result<RuntimeDecoderSessionHandle>;
    fn info(&mut self, session: RuntimeDecoderSessionHandle) -> Result<RuntimeDecoderInfo>;
    fn metadata(&mut self, session: RuntimeDecoderSessionHandle) -> Result<RuntimeMediaMetadata>;
    fn read_pcm_f32(
        &mut self,
        session: RuntimeDecoderSessionHandle,
        max_frames: u32,
    ) -> Result<RuntimePcmF32Chunk>;
    fn seek_ms(&mut self, session: RuntimeDecoderSessionHandle, position_ms: u64) -> Result<()>;
    fn close(&mut self, session: RuntimeDecoderSessionHandle) -> Result<()>;
}

pub struct WasmtimeDecoderPlugin {
    plugin_id: String,
    component: PluginCell<Store<DecoderStoreData>, DecoderBinding>,
    next_session_handle: u64,
    sessions: BTreeMap<u64, wasmtime::component::ResourceAny>,
}

impl WasmtimeDecoderPlugin {
    pub fn plugin_id(&self) -> &str {
        &self.plugin_id
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

    fn reconcile_runtime(&mut self) -> Result<()> {
        let session_refs = self.sessions.values().cloned().collect::<Vec<_>>();
        let mut rebuilt = false;
        let mut destroyed = false;
        reconcile_with(
            &mut self.component,
            |store, plugin, config_json| {
                let decoder = plugin.stellatune_plugin_decoder();
                for session in &session_refs {
                    let plan = map_decoder_plugin_error(
                        decoder.session().call_plan_config_update_json(
                            &mut *store,
                            *session,
                            config_json,
                        )?,
                        "decoder.session.plan-config-update-json",
                    )?;
                    match plan.mode {
                        decoder_common::ConfigUpdateMode::HotApply => {
                            map_decoder_plugin_error(
                                decoder.session().call_apply_config_update_json(
                                    &mut *store,
                                    *session,
                                    config_json,
                                )?,
                                "decoder.session.apply-config-update-json",
                            )?;
                        },
                        decoder_common::ConfigUpdateMode::Recreate => {
                            return Err(crate::op_error!(
                                "decoder session requested recreate for config update"
                            ));
                        },
                        decoder_common::ConfigUpdateMode::Reject => {
                            return Err(crate::op_error!(
                                "decoder session rejected config update: {}",
                                plan.reason.unwrap_or_else(|| "unknown".to_string())
                            ));
                        },
                    }
                }
                Ok(())
            },
            |store, plugin| {
                let decoder = plugin.stellatune_plugin_decoder();
                for session in &session_refs {
                    let _ = decoder.session().call_close(&mut *store, *session);
                    let _ = (*session).resource_drop(&mut *store);
                }
                let disable = plugin
                    .stellatune_plugin_lifecycle()
                    .call_on_disable(
                        &mut *store,
                        map_disable_reason_decoder(PluginDisableReason::Reload),
                    )
                    .map_err(|error| {
                        crate::op_error!("lifecycle.on-disable call failed: {error:#}")
                    })?;
                disable.map_err(|error| {
                    crate::op_error!("lifecycle.on-disable plugin error: {error:?}")
                })?;
                let enable = plugin
                    .stellatune_plugin_lifecycle()
                    .call_on_enable(&mut *store)
                    .map_err(|error| {
                        crate::op_error!("lifecycle.on-enable call failed: {error:#}")
                    })?;
                enable.map_err(|error| {
                    crate::op_error!("lifecycle.on-enable plugin error: {error:?}")
                })?;
                rebuilt = true;
                Ok(())
            },
            |store, plugin, reason| {
                let decoder = plugin.stellatune_plugin_decoder();
                for session in &session_refs {
                    let _ = decoder.session().call_close(&mut *store, *session);
                    let _ = (*session).resource_drop(&mut *store);
                }
                let disable = plugin
                    .stellatune_plugin_lifecycle()
                    .call_on_disable(&mut *store, map_disable_reason_decoder(reason))
                    .map_err(|error| {
                        crate::op_error!("lifecycle.on-disable call failed: {error:#}")
                    })?;
                disable.map_err(|error| {
                    crate::op_error!("lifecycle.on-disable plugin error: {error:?}")
                })?;
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

impl DecoderPluginApi for WasmtimeDecoderPlugin {
    fn open_stream(
        &mut self,
        stream: Box<dyn HostStreamHandle>,
        ext_hint: Option<&str>,
    ) -> Result<RuntimeDecoderSessionHandle> {
        self.reconcile_runtime()?;
        let rep = {
            let state = self.component.store.data_mut();
            let rep = state.alloc_rep();
            state.streams.insert(rep, stream);
            rep
        };
        let host_stream = Resource::new_own(rep);
        let decoder = self.component.plugin.stellatune_plugin_decoder();
        let open_result = decoder.call_open(&mut self.component.store, host_stream, ext_hint);
        let session = match open_result {
            Ok(result) => match map_decoder_plugin_error(result, "decoder.open") {
                Ok(session) => session,
                Err(error) => {
                    self.component.store.data_mut().streams.remove(&rep);
                    return Err(error);
                },
            },
            Err(error) => {
                self.component.store.data_mut().streams.remove(&rep);
                return Err(error.into());
            },
        };
        let handle = self.alloc_session_handle();
        self.sessions.insert(handle, session);
        Ok(RuntimeDecoderSessionHandle(handle))
    }

    fn info(&mut self, session: RuntimeDecoderSessionHandle) -> Result<RuntimeDecoderInfo> {
        let Some(session_ref) = self.sessions.get(&session.0).cloned() else {
            return Err(crate::op_error!(
                "decoder session `{}` not found",
                session.0
            ));
        };

        self.reconcile_runtime()?;
        let decoder = self.component.plugin.stellatune_plugin_decoder();
        let info = map_decoder_plugin_error(
            decoder
                .session()
                .call_info(&mut self.component.store, session_ref)?,
            "decoder.session.info",
        )?;
        Ok(runtime_decoder_info_from!(info))
    }

    fn metadata(&mut self, session: RuntimeDecoderSessionHandle) -> Result<RuntimeMediaMetadata> {
        let Some(session_ref) = self.sessions.get(&session.0).cloned() else {
            return Err(crate::op_error!(
                "decoder session `{}` not found",
                session.0
            ));
        };

        self.reconcile_runtime()?;
        let decoder = self.component.plugin.stellatune_plugin_decoder();
        let meta = map_decoder_plugin_error(
            decoder
                .session()
                .call_metadata(&mut self.component.store, session_ref)?,
            "decoder.session.metadata",
        )?;
        Ok(runtime_media_metadata_from_decoder!(meta))
    }

    fn read_pcm_f32(
        &mut self,
        session: RuntimeDecoderSessionHandle,
        max_frames: u32,
    ) -> Result<RuntimePcmF32Chunk> {
        let Some(session_ref) = self.sessions.get(&session.0).cloned() else {
            return Err(crate::op_error!(
                "decoder session `{}` not found",
                session.0
            ));
        };

        self.reconcile_runtime()?;
        let decoder = self.component.plugin.stellatune_plugin_decoder();
        let chunk = map_decoder_plugin_error(
            decoder.session().call_read_pcm_f32(
                &mut self.component.store,
                session_ref,
                max_frames,
            )?,
            "decoder.session.read-pcm-f32",
        )?;
        Ok(runtime_pcm_chunk_from!(chunk))
    }

    fn seek_ms(&mut self, session: RuntimeDecoderSessionHandle, position_ms: u64) -> Result<()> {
        let Some(session_ref) = self.sessions.get(&session.0).cloned() else {
            return Err(crate::op_error!(
                "decoder session `{}` not found",
                session.0
            ));
        };

        self.reconcile_runtime()?;
        let decoder = self.component.plugin.stellatune_plugin_decoder();
        map_decoder_plugin_error(
            decoder
                .session()
                .call_seek_ms(&mut self.component.store, session_ref, position_ms)?,
            "decoder.session.seek-ms",
        )?;
        Ok(())
    }

    fn close(&mut self, session: RuntimeDecoderSessionHandle) -> Result<()> {
        let Some(session_ref) = self.sessions.remove(&session.0) else {
            return Ok(());
        };
        let decoder = self.component.plugin.stellatune_plugin_decoder();
        let _ = decoder
            .session()
            .call_close(&mut self.component.store, session_ref);
        let _ = session_ref.resource_drop(&mut self.component.store);
        Ok(())
    }
}

impl Drop for WasmtimeDecoderPlugin {
    fn drop(&mut self) {
        let sessions = std::mem::take(&mut self.sessions);
        let decoder = self.component.plugin.stellatune_plugin_decoder();
        for (_, session_ref) in sessions {
            let _ = decoder
                .session()
                .call_close(&mut self.component.store, session_ref);
            let _ = session_ref.resource_drop(&mut self.component.store);
        }
        if self.component.state() != PluginCellState::Destroyed {
            let _ = self
                .component
                .plugin
                .stellatune_plugin_lifecycle()
                .call_on_disable(
                    &mut self.component.store,
                    map_disable_reason_decoder(PluginDisableReason::HostDisable),
                );
        }
    }
}

impl WasmtimePluginController {
    pub fn create_decoder_plugin(
        &self,
        plugin_id: &str,
        type_id: &str,
    ) -> Result<WasmtimeDecoderPlugin> {
        let (plugin, capability) =
            self.resolve_capability(plugin_id, AbilityKind::Decoder, type_id)?;
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
            WorldKind::Decoder => {
                self.instantiate_decoder_component(&plugin.root_dir, &component, rx)?
            },
            _ => {
                return Err(crate::op_error!(
                    "capability world `{}` is not a decoder world",
                    capability.world
                ));
            },
        };

        self.register_directive_sender(plugin_id, tx)?;

        Ok(WasmtimeDecoderPlugin {
            plugin_id: plugin_id.to_string(),
            component,
            next_session_handle: 1,
            sessions: BTreeMap::new(),
        })
    }

    pub fn install_and_create_decoder_plugin(
        &self,
        plugin: &RuntimePluginInfo,
        capabilities: &[RuntimeCapabilityDescriptor],
        type_id: &str,
    ) -> Result<WasmtimeDecoderPlugin> {
        WasmPluginController::install_plugin(self, plugin, capabilities)?;
        self.create_decoder_plugin(&plugin.id, type_id)
    }
}
