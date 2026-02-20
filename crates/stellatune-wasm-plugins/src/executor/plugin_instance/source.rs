use std::collections::BTreeMap;
use std::sync::mpsc;

use crate::error::Result;
use wasmtime::Store;
use wasmtime::component::Component;

use stellatune_wasm_host_bindings::generated as host_bindings;

use host_bindings::source_plugin::SourcePlugin as SourceBinding;
use host_bindings::source_plugin::exports::stellatune::plugin::source as source_exports;
use host_bindings::source_plugin::stellatune::plugin::common as source_common;

use crate::executor::plugin_cell::{PluginCell, PluginCellState};
use crate::executor::stores::source::SourceStoreData;
use crate::executor::{
    WasmPluginController, WasmtimePluginController, WorldKind, classify_world,
    map_disable_reason_source,
};
use crate::manifest::AbilityKind;
use crate::runtime::model::{
    PluginDisableReason, RuntimeAudioTags, RuntimeCapabilityDescriptor, RuntimeEncodedAudioFormat,
    RuntimeEncodedChunk, RuntimeMediaMetadata, RuntimeMetadataEntry, RuntimeMetadataValue,
    RuntimePluginDirective, RuntimePluginInfo, RuntimeSourceStreamHandle,
};

use crate::executor::plugin_instance::common::reconcile_with;

pub trait SourcePluginApi {
    fn list_items_json(&mut self, request_json: &str) -> Result<String>;
    fn open_stream_json(&mut self, track_json: &str) -> Result<RuntimeSourceStreamHandle>;
    fn open_uri(&mut self, uri: &str) -> Result<RuntimeSourceStreamHandle>;
    fn metadata(&mut self, stream: RuntimeSourceStreamHandle) -> Result<RuntimeMediaMetadata>;
    fn read(
        &mut self,
        stream: RuntimeSourceStreamHandle,
        max_bytes: u32,
    ) -> Result<RuntimeEncodedChunk>;
    fn close_stream(&mut self, stream: RuntimeSourceStreamHandle) -> Result<()>;
    fn plan_config_update_json(&mut self, config_json: &str) -> Result<(String, Option<String>)>;
    fn apply_config_update_json(&mut self, config_json: &str) -> Result<()>;
    fn export_state_json(&mut self) -> Result<Option<String>>;
    fn import_state_json(&mut self, state_json: &str) -> Result<()>;
}

pub struct WasmtimeSourcePlugin {
    plugin_id: String,
    component: PluginCell<Store<SourceStoreData>, SourceBinding>,
    catalog: Option<wasmtime::component::ResourceAny>,
    next_stream_handle: u64,
    streams: BTreeMap<u64, wasmtime::component::ResourceAny>,
}

impl WasmtimeSourcePlugin {
    pub fn plugin_id(&self) -> &str {
        &self.plugin_id
    }

    fn alloc_stream_handle(&mut self) -> u64 {
        let handle = if self.next_stream_handle == 0 {
            1
        } else {
            self.next_stream_handle
        };
        self.next_stream_handle = handle.saturating_add(1);
        if self.next_stream_handle == 0 {
            self.next_stream_handle = 1;
        }
        handle
    }

    fn source_api(&self) -> source_exports::Guest {
        self.component.plugin.stellatune_plugin_source().clone()
    }

    fn reconcile_runtime(&mut self) -> Result<()> {
        let stream_refs = self.streams.values().cloned().collect::<Vec<_>>();
        let catalog = self.catalog;
        let mut rebuilt = false;
        let mut destroyed = false;
        reconcile_with(
            &mut self.component,
            |store, plugin, config_json| {
                let source = plugin.stellatune_plugin_source();
                if let Some(catalog_ref) = catalog {
                    let plan = source
                        .catalog()
                        .call_plan_config_update_json(&mut *store, catalog_ref, config_json)?
                        .map_err(|error| {
                            crate::op_error!(
                                "source.catalog.plan-config-update-json plugin error: {error:?}"
                            )
                        })?;
                    match plan.mode {
                        source_common::ConfigUpdateMode::HotApply => {
                            source
                                .catalog()
                                .call_apply_config_update_json(
                                    &mut *store,
                                    catalog_ref,
                                    config_json,
                                )?
                                .map_err(|error| crate::op_error!("source.catalog.apply-config-update-json plugin error: {error:?}"))?;
                        },
                        source_common::ConfigUpdateMode::Recreate => {
                            return Err(crate::op_error!(
                                "source catalog requested recreate for config update"
                            ));
                        },
                        source_common::ConfigUpdateMode::Reject => {
                            return Err(crate::op_error!(
                                "source catalog rejected config update: {}",
                                plan.reason.unwrap_or_else(|| "unknown".to_string())
                            ));
                        },
                    }
                }
                Ok(())
            },
            |store, plugin| {
                let source = plugin.stellatune_plugin_source();
                for stream in &stream_refs {
                    let _ = source.source_stream().call_close(&mut *store, *stream);
                    let _ = (*stream).resource_drop(&mut *store);
                }
                if let Some(catalog_ref) = catalog {
                    let _ = source.catalog().call_close(&mut *store, catalog_ref);
                    let _ = catalog_ref.resource_drop(&mut *store);
                }
                let disable = plugin
                    .stellatune_plugin_lifecycle()
                    .call_on_disable(
                        &mut *store,
                        map_disable_reason_source(PluginDisableReason::Reload),
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
                let source = plugin.stellatune_plugin_source();
                for stream in &stream_refs {
                    let _ = source.source_stream().call_close(&mut *store, *stream);
                    let _ = (*stream).resource_drop(&mut *store);
                }
                if let Some(catalog_ref) = catalog {
                    let _ = source.catalog().call_close(&mut *store, catalog_ref);
                    let _ = catalog_ref.resource_drop(&mut *store);
                }
                let disable = plugin
                    .stellatune_plugin_lifecycle()
                    .call_on_disable(&mut *store, map_disable_reason_source(reason))
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
            self.catalog = None;
            self.streams.clear();
        }
        Ok(())
    }

    fn ensure_catalog(&mut self) -> Result<wasmtime::component::ResourceAny> {
        if let Some(catalog) = self.catalog {
            return Ok(catalog);
        }
        let source = self.source_api();
        let catalog = source
            .call_create(&mut self.component.store)?
            .map_err(|error| crate::op_error!("source.create plugin error: {error:?}"))?;
        self.catalog = Some(catalog);
        self.catalog
            .ok_or_else(|| crate::op_error!("source catalog handle missing after create"))
    }
}

impl SourcePluginApi for WasmtimeSourcePlugin {
    fn list_items_json(&mut self, request_json: &str) -> Result<String> {
        self.reconcile_runtime()?;
        let catalog = self.ensure_catalog()?;
        let source = self.source_api();
        source
            .catalog()
            .call_list_items_json(&mut self.component.store, catalog, request_json)?
            .map_err(|error| {
                crate::op_error!("source.catalog.list-items-json plugin error: {error:?}")
            })
    }

    fn open_stream_json(&mut self, track_json: &str) -> Result<RuntimeSourceStreamHandle> {
        self.reconcile_runtime()?;
        let catalog = self.ensure_catalog()?;
        let source = self.source_api();
        let stream = source
            .catalog()
            .call_open_stream_json(&mut self.component.store, catalog, track_json)?
            .map_err(|error| {
                crate::op_error!("source.catalog.open-stream-json plugin error: {error:?}")
            })?;
        let handle = self.alloc_stream_handle();
        self.streams.insert(handle, stream);
        Ok(RuntimeSourceStreamHandle(handle))
    }

    fn open_uri(&mut self, uri: &str) -> Result<RuntimeSourceStreamHandle> {
        let uri = uri.trim();
        if uri.is_empty() {
            return Err(crate::op_error!("source uri is empty"));
        }
        self.reconcile_runtime()?;
        let catalog = self.ensure_catalog()?;
        let source = self.source_api();
        let stream = source
            .catalog()
            .call_open_uri(&mut self.component.store, catalog, uri)?
            .map_err(|error| crate::op_error!("source.catalog.open-uri plugin error: {error:?}"))?;
        let handle = self.alloc_stream_handle();
        self.streams.insert(handle, stream);
        Ok(RuntimeSourceStreamHandle(handle))
    }

    fn metadata(&mut self, stream: RuntimeSourceStreamHandle) -> Result<RuntimeMediaMetadata> {
        let Some(stream_ref) = self.streams.get(&stream.0).cloned() else {
            return Err(crate::op_error!("source stream `{}` not found", stream.0));
        };
        self.reconcile_runtime()?;
        let source = self.source_api();
        let meta = source
            .source_stream()
            .call_metadata(&mut self.component.store, stream_ref)?
            .map_err(|error| crate::op_error!("source.stream.metadata plugin error: {error:?}"))?;
        Ok(RuntimeMediaMetadata {
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
                        source_common::MetadataValue::Text(v) => RuntimeMetadataValue::Text(v),
                        source_common::MetadataValue::Boolean(v) => {
                            RuntimeMetadataValue::Boolean(v)
                        },
                        source_common::MetadataValue::Uint32(v) => RuntimeMetadataValue::Uint32(v),
                        source_common::MetadataValue::Uint64(v) => RuntimeMetadataValue::Uint64(v),
                        source_common::MetadataValue::Int64(v) => RuntimeMetadataValue::Int64(v),
                        source_common::MetadataValue::Float64(v) => {
                            RuntimeMetadataValue::Float64(v)
                        },
                        source_common::MetadataValue::Bytes(v) => RuntimeMetadataValue::Bytes(v),
                    },
                })
                .collect::<Vec<_>>(),
        })
    }

    fn read(
        &mut self,
        stream: RuntimeSourceStreamHandle,
        max_bytes: u32,
    ) -> Result<RuntimeEncodedChunk> {
        let Some(stream_ref) = self.streams.get(&stream.0).cloned() else {
            return Err(crate::op_error!("source stream `{}` not found", stream.0));
        };
        self.reconcile_runtime()?;
        let source = self.source_api();
        let chunk = source
            .source_stream()
            .call_read(&mut self.component.store, stream_ref, max_bytes)?
            .map_err(|error| crate::op_error!("source.stream.read plugin error: {error:?}"))?;
        Ok(RuntimeEncodedChunk {
            bytes: chunk.bytes,
            eof: chunk.eof,
        })
    }

    fn close_stream(&mut self, stream: RuntimeSourceStreamHandle) -> Result<()> {
        let Some(stream_ref) = self.streams.remove(&stream.0) else {
            return Ok(());
        };
        let source = self.source_api();
        let _ = source
            .source_stream()
            .call_close(&mut self.component.store, stream_ref);
        let _ = stream_ref.resource_drop(&mut self.component.store);
        Ok(())
    }

    fn plan_config_update_json(&mut self, config_json: &str) -> Result<(String, Option<String>)> {
        self.reconcile_runtime()?;
        let catalog = self.ensure_catalog()?;
        let source = self.source_api();
        let plan = source
            .catalog()
            .call_plan_config_update_json(&mut self.component.store, catalog, config_json)?
            .map_err(|error| {
                crate::op_error!("source.catalog.plan-config-update-json plugin error: {error:?}")
            })?;
        Ok((format!("{:?}", plan.mode), plan.reason))
    }

    fn apply_config_update_json(&mut self, config_json: &str) -> Result<()> {
        self.reconcile_runtime()?;
        let catalog = self.ensure_catalog()?;
        let source = self.source_api();
        source
            .catalog()
            .call_apply_config_update_json(&mut self.component.store, catalog, config_json)?
            .map_err(|error| {
                crate::op_error!("source.catalog.apply-config-update-json plugin error: {error:?}")
            })?;
        Ok(())
    }

    fn export_state_json(&mut self) -> Result<Option<String>> {
        self.reconcile_runtime()?;
        let catalog = self.ensure_catalog()?;
        let source = self.source_api();
        source
            .catalog()
            .call_export_state_json(&mut self.component.store, catalog)?
            .map_err(|error| {
                crate::op_error!("source.catalog.export-state-json plugin error: {error:?}")
            })
    }

    fn import_state_json(&mut self, state_json: &str) -> Result<()> {
        self.reconcile_runtime()?;
        let catalog = self.ensure_catalog()?;
        let source = self.source_api();
        source
            .catalog()
            .call_import_state_json(&mut self.component.store, catalog, state_json)?
            .map_err(|error| {
                crate::op_error!("source.catalog.import-state-json plugin error: {error:?}")
            })?;
        Ok(())
    }
}

impl Drop for WasmtimeSourcePlugin {
    fn drop(&mut self) {
        let streams = std::mem::take(&mut self.streams);
        let source = self.source_api();
        for (_, stream_ref) in streams {
            let _ = source
                .source_stream()
                .call_close(&mut self.component.store, stream_ref);
            let _ = stream_ref.resource_drop(&mut self.component.store);
        }
        if let Some(catalog) = self.catalog.take() {
            let _ = source
                .catalog()
                .call_close(&mut self.component.store, catalog);
            let _ = catalog.resource_drop(&mut self.component.store);
        }
        if self.component.state() != PluginCellState::Destroyed {
            let _ = self
                .component
                .plugin
                .stellatune_plugin_lifecycle()
                .call_on_disable(
                    &mut self.component.store,
                    map_disable_reason_source(PluginDisableReason::HostDisable),
                );
        }
    }
}

impl WasmtimePluginController {
    pub fn create_source_plugin(
        &self,
        plugin_id: &str,
        type_id: &str,
    ) -> Result<WasmtimeSourcePlugin> {
        let (plugin, capability) =
            self.resolve_capability(plugin_id, AbilityKind::Source, type_id)?;
        let plugin_id = plugin.id.trim();
        self.ensure_plugin_active(plugin_id)?;

        let component_path = plugin.root_dir.join(&capability.component_rel_path);
        let component = Component::from_file(&self.engine, &component_path).map_err(|error| {
            crate::op_error!(
                "failed to load component for plugin `{}` component `{}`: {error:#}",
                plugin_id,
                capability.component_id
            )
        })?;

        let (tx, rx) = mpsc::channel::<RuntimePluginDirective>();
        let component = match classify_world(&capability.world) {
            WorldKind::Source => self.instantiate_source_component(&component, rx)?,
            _ => {
                return Err(crate::op_error!(
                    "capability world `{}` is not a source world",
                    capability.world
                ));
            },
        };
        self.register_directive_sender(plugin_id, tx)?;

        Ok(WasmtimeSourcePlugin {
            plugin_id: plugin_id.to_string(),
            component,
            catalog: None,
            next_stream_handle: 1,
            streams: BTreeMap::new(),
        })
    }

    pub fn install_and_create_source_plugin(
        &self,
        plugin: &RuntimePluginInfo,
        capabilities: &[RuntimeCapabilityDescriptor],
        type_id: &str,
    ) -> Result<WasmtimeSourcePlugin> {
        WasmPluginController::install_plugin(self, plugin, capabilities)?;
        self.create_source_plugin(&plugin.id, type_id)
    }
}
