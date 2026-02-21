use std::collections::BTreeMap;
use std::sync::mpsc;

use crate::error::Result;
use wasmtime::Store;

use stellatune_host_bindings::generated as host_bindings;

use host_bindings::dsp_plugin::DspPlugin as DspBinding;
use host_bindings::dsp_plugin::exports::stellatune::plugin::dsp as dsp_exports;
use host_bindings::dsp_plugin::stellatune::plugin::common as dsp_common;
use host_bindings::dsp_plugin::stellatune::plugin::hot_path as dsp_hot_path;

use crate::executor::plugin_cell::{PluginCell, PluginCellState};
use crate::executor::stores::dsp::DspStoreData;
use crate::executor::{
    WasmPluginController, WasmtimePluginController, WorldKind, call_dsp_on_disable,
    call_dsp_on_enable, classify_world, map_disable_reason_dsp,
};
use crate::manifest::AbilityKind;
use crate::runtime::model::{
    PluginDisableReason, RuntimeAudioSpec, RuntimeBufferLayout, RuntimeCapabilityDescriptor,
    RuntimeCoreModuleSpec, RuntimeDspProcessorHandle, RuntimeHotPathRole, RuntimePluginDirective,
    RuntimePluginInfo, RuntimeSampleFormat,
};

use crate::executor::plugin_instance::common::reconcile_with;

pub trait DspPluginApi {
    fn create_processor(&mut self, spec: RuntimeAudioSpec) -> Result<RuntimeDspProcessorHandle>;
    fn describe_hot_path(
        &mut self,
        processor: RuntimeDspProcessorHandle,
        spec: RuntimeAudioSpec,
    ) -> Result<Option<RuntimeCoreModuleSpec>>;
    fn process_interleaved_f32(
        &mut self,
        processor: RuntimeDspProcessorHandle,
        channels: u16,
        interleaved_f32le: Vec<u8>,
    ) -> Result<Vec<u8>>;
    fn supported_layouts(&mut self, processor: RuntimeDspProcessorHandle) -> Result<u32>;
    fn output_channels(&mut self, processor: RuntimeDspProcessorHandle) -> Result<u16>;
    fn plan_config_update_json(
        &mut self,
        processor: RuntimeDspProcessorHandle,
        config_json: &str,
    ) -> Result<(String, Option<String>)>;
    fn apply_config_update_json(
        &mut self,
        processor: RuntimeDspProcessorHandle,
        config_json: &str,
    ) -> Result<()>;
    fn export_state_json(&mut self, processor: RuntimeDspProcessorHandle)
    -> Result<Option<String>>;
    fn import_state_json(
        &mut self,
        processor: RuntimeDspProcessorHandle,
        state_json: &str,
    ) -> Result<()>;
    fn close_processor(&mut self, processor: RuntimeDspProcessorHandle) -> Result<()>;
}

pub struct WasmtimeDspPlugin {
    plugin_id: String,
    component: PluginCell<Store<DspStoreData>, DspBinding>,
    next_processor_handle: u64,
    processors: BTreeMap<u64, wasmtime::component::ResourceAny>,
}

impl WasmtimeDspPlugin {
    pub fn plugin_id(&self) -> &str {
        &self.plugin_id
    }

    fn alloc_processor_handle(&mut self) -> u64 {
        let handle = if self.next_processor_handle == 0 {
            1
        } else {
            self.next_processor_handle
        };
        self.next_processor_handle = handle.saturating_add(1);
        if self.next_processor_handle == 0 {
            self.next_processor_handle = 1;
        }
        handle
    }

    fn dsp_api(&self) -> dsp_exports::Guest {
        self.component.plugin.stellatune_plugin_dsp().clone()
    }

    fn reconcile_runtime(&mut self) -> Result<()> {
        let processor_refs = self.processors.values().cloned().collect::<Vec<_>>();
        let mut rebuilt = false;
        let mut destroyed = false;
        reconcile_with(
            &mut self.component,
            |store, plugin, config_json| {
                let dsp = plugin.stellatune_plugin_dsp();
                for processor in &processor_refs {
                    let plan = dsp
                        .processor()
                        .call_plan_config_update_json(&mut *store, *processor, config_json)?
                        .map_err(|error| {
                            crate::op_error!(
                                "dsp.processor.plan-config-update-json plugin error: {error:?}"
                            )
                        })?;
                    match plan.mode {
                        dsp_common::ConfigUpdateMode::HotApply => {
                            dsp.processor()
                                .call_apply_config_update_json(
                                    &mut *store,
                                    *processor,
                                    config_json,
                                )?
                                .map_err(|error| crate::op_error!("dsp.processor.apply-config-update-json plugin error: {error:?}"))?;
                        },
                        dsp_common::ConfigUpdateMode::Recreate => {
                            return Err(crate::op_error!(
                                "dsp processor requested recreate for config update"
                            ));
                        },
                        dsp_common::ConfigUpdateMode::Reject => {
                            return Err(crate::op_error!(
                                "dsp processor rejected config update: {}",
                                plan.reason.unwrap_or_else(|| "unknown".to_string())
                            ));
                        },
                    }
                }
                Ok(())
            },
            |store, plugin| {
                let dsp = plugin.stellatune_plugin_dsp();
                for processor in &processor_refs {
                    let _ = dsp.processor().call_close(&mut *store, *processor);
                    let _ = (*processor).resource_drop(&mut *store);
                }
                call_dsp_on_disable(
                    plugin,
                    store,
                    map_disable_reason_dsp(PluginDisableReason::Reload),
                )?;
                call_dsp_on_enable(plugin, store)?;
                rebuilt = true;
                Ok(())
            },
            |store, plugin, reason| {
                let dsp = plugin.stellatune_plugin_dsp();
                for processor in &processor_refs {
                    let _ = dsp.processor().call_close(&mut *store, *processor);
                    let _ = (*processor).resource_drop(&mut *store);
                }
                call_dsp_on_disable(plugin, store, map_disable_reason_dsp(reason))?;
                destroyed = true;
                Ok(())
            },
        )?;
        if rebuilt || destroyed {
            self.processors.clear();
        }
        Ok(())
    }
}

impl DspPluginApi for WasmtimeDspPlugin {
    fn create_processor(&mut self, spec: RuntimeAudioSpec) -> Result<RuntimeDspProcessorHandle> {
        self.reconcile_runtime()?;
        let dsp = self.dsp_api();
        let processor = dsp
            .call_create(
                &mut self.component.store,
                dsp_common::AudioSpec {
                    sample_rate: spec.sample_rate,
                    channels: spec.channels,
                },
            )?
            .map_err(|error| crate::op_error!("dsp.create plugin error: {error:?}"))?;
        let handle = self.alloc_processor_handle();
        self.processors.insert(handle, processor);
        Ok(RuntimeDspProcessorHandle(handle))
    }

    fn describe_hot_path(
        &mut self,
        processor: RuntimeDspProcessorHandle,
        spec: RuntimeAudioSpec,
    ) -> Result<Option<RuntimeCoreModuleSpec>> {
        let Some(processor_ref) = self.processors.get(&processor.0).cloned() else {
            return Err(crate::op_error!(
                "dsp processor `{}` not found",
                processor.0
            ));
        };
        self.reconcile_runtime()?;
        let dsp = self.dsp_api();
        let maybe_spec = dsp
            .processor()
            .call_describe_hot_path(
                &mut self.component.store,
                processor_ref,
                dsp_common::AudioSpec {
                    sample_rate: spec.sample_rate,
                    channels: spec.channels,
                },
            )?
            .map_err(|error| {
                crate::op_error!("dsp.processor.describe-hot-path plugin error: {error:?}")
            })?;
        Ok(maybe_spec.map(|spec| RuntimeCoreModuleSpec {
            role: match spec.role {
                dsp_hot_path::Role::DspTransform => RuntimeHotPathRole::DspTransform,
                dsp_hot_path::Role::OutputSink => RuntimeHotPathRole::OutputSink,
            },
            wasm_rel_path: spec.wasm_rel_path,
            abi_version: spec.abi_version,
            memory_export: spec.memory_export,
            init_export: spec.init_export,
            process_export: spec.process_export,
            reset_export: spec.reset_export,
            drop_export: spec.drop_export,
            buffer: RuntimeBufferLayout {
                in_offset: spec.buffer.in_offset,
                out_offset: spec.buffer.out_offset,
                max_frames: spec.buffer.max_frames,
                channels: spec.buffer.channels,
                sample_format: match spec.buffer.sample_format {
                    dsp_hot_path::SampleFormat::F32le => RuntimeSampleFormat::F32Le,
                    dsp_hot_path::SampleFormat::I16le => RuntimeSampleFormat::I16Le,
                    dsp_hot_path::SampleFormat::I32le => RuntimeSampleFormat::I32Le,
                },
                interleaved: spec.buffer.interleaved,
            },
        }))
    }

    fn process_interleaved_f32(
        &mut self,
        processor: RuntimeDspProcessorHandle,
        channels: u16,
        interleaved_f32le: Vec<u8>,
    ) -> Result<Vec<u8>> {
        let Some(processor_ref) = self.processors.get(&processor.0).cloned() else {
            return Err(crate::op_error!(
                "dsp processor `{}` not found",
                processor.0
            ));
        };
        self.reconcile_runtime()?;
        let dsp = self.dsp_api();
        dsp.processor()
            .call_process_interleaved_f32(
                &mut self.component.store,
                processor_ref,
                channels,
                &interleaved_f32le,
            )?
            .map_err(|error| {
                crate::op_error!("dsp.processor.process-interleaved-f32 plugin error: {error:?}")
            })
    }

    fn supported_layouts(&mut self, processor: RuntimeDspProcessorHandle) -> Result<u32> {
        let Some(processor_ref) = self.processors.get(&processor.0).cloned() else {
            return Err(crate::op_error!(
                "dsp processor `{}` not found",
                processor.0
            ));
        };
        self.reconcile_runtime()?;
        let dsp = self.dsp_api();
        Ok(dsp
            .processor()
            .call_supported_layouts(&mut self.component.store, processor_ref)?)
    }

    fn output_channels(&mut self, processor: RuntimeDspProcessorHandle) -> Result<u16> {
        let Some(processor_ref) = self.processors.get(&processor.0).cloned() else {
            return Err(crate::op_error!(
                "dsp processor `{}` not found",
                processor.0
            ));
        };
        self.reconcile_runtime()?;
        let dsp = self.dsp_api();
        Ok(dsp
            .processor()
            .call_output_channels(&mut self.component.store, processor_ref)?)
    }

    fn plan_config_update_json(
        &mut self,
        processor: RuntimeDspProcessorHandle,
        config_json: &str,
    ) -> Result<(String, Option<String>)> {
        let Some(processor_ref) = self.processors.get(&processor.0).cloned() else {
            return Err(crate::op_error!(
                "dsp processor `{}` not found",
                processor.0
            ));
        };
        self.reconcile_runtime()?;
        let dsp = self.dsp_api();
        let plan = dsp
            .processor()
            .call_plan_config_update_json(&mut self.component.store, processor_ref, config_json)?
            .map_err(|error| {
                crate::op_error!("dsp.processor.plan-config-update-json plugin error: {error:?}")
            })?;
        Ok((format!("{:?}", plan.mode), plan.reason))
    }

    fn apply_config_update_json(
        &mut self,
        processor: RuntimeDspProcessorHandle,
        config_json: &str,
    ) -> Result<()> {
        let Some(processor_ref) = self.processors.get(&processor.0).cloned() else {
            return Err(crate::op_error!(
                "dsp processor `{}` not found",
                processor.0
            ));
        };
        self.reconcile_runtime()?;
        let dsp = self.dsp_api();
        dsp.processor()
            .call_apply_config_update_json(&mut self.component.store, processor_ref, config_json)?
            .map_err(|error| {
                crate::op_error!("dsp.processor.apply-config-update-json plugin error: {error:?}")
            })?;
        Ok(())
    }

    fn export_state_json(
        &mut self,
        processor: RuntimeDspProcessorHandle,
    ) -> Result<Option<String>> {
        let Some(processor_ref) = self.processors.get(&processor.0).cloned() else {
            return Err(crate::op_error!(
                "dsp processor `{}` not found",
                processor.0
            ));
        };
        self.reconcile_runtime()?;
        let dsp = self.dsp_api();
        dsp.processor()
            .call_export_state_json(&mut self.component.store, processor_ref)?
            .map_err(|error| {
                crate::op_error!("dsp.processor.export-state-json plugin error: {error:?}")
            })
    }

    fn import_state_json(
        &mut self,
        processor: RuntimeDspProcessorHandle,
        state_json: &str,
    ) -> Result<()> {
        let Some(processor_ref) = self.processors.get(&processor.0).cloned() else {
            return Err(crate::op_error!(
                "dsp processor `{}` not found",
                processor.0
            ));
        };
        self.reconcile_runtime()?;
        let dsp = self.dsp_api();
        dsp.processor()
            .call_import_state_json(&mut self.component.store, processor_ref, state_json)?
            .map_err(|error| {
                crate::op_error!("dsp.processor.import-state-json plugin error: {error:?}")
            })?;
        Ok(())
    }

    fn close_processor(&mut self, processor: RuntimeDspProcessorHandle) -> Result<()> {
        let Some(processor_ref) = self.processors.remove(&processor.0) else {
            return Ok(());
        };
        let dsp = self.dsp_api();
        let _ = dsp
            .processor()
            .call_close(&mut self.component.store, processor_ref);
        let _ = processor_ref.resource_drop(&mut self.component.store);
        Ok(())
    }
}

impl Drop for WasmtimeDspPlugin {
    fn drop(&mut self) {
        let processors = std::mem::take(&mut self.processors);
        let dsp = self.dsp_api();
        for (_, processor_ref) in processors {
            let _ = dsp
                .processor()
                .call_close(&mut self.component.store, processor_ref);
            let _ = processor_ref.resource_drop(&mut self.component.store);
        }
        if self.component.state() != PluginCellState::Destroyed {
            let _ = call_dsp_on_disable(
                &self.component.plugin,
                &mut self.component.store,
                map_disable_reason_dsp(PluginDisableReason::HostDisable),
            );
        }
    }
}

impl WasmtimePluginController {
    pub fn create_dsp_plugin(&self, plugin_id: &str, type_id: &str) -> Result<WasmtimeDspPlugin> {
        let (plugin, capability) = self.resolve_capability(plugin_id, AbilityKind::Dsp, type_id)?;
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
        let component: PluginCell<Store<DspStoreData>, DspBinding> =
            match classify_world(&capability.world) {
                WorldKind::Dsp => {
                    self.instantiate_dsp_component(plugin_id, &plugin.root_dir, &component, rx)?
                },
                _ => {
                    return Err(crate::op_error!(
                        "capability world `{}` is not a dsp world",
                        capability.world
                    ));
                },
            };
        self.register_directive_sender(plugin_id, tx)?;

        Ok(WasmtimeDspPlugin {
            plugin_id: plugin_id.to_string(),
            component,
            next_processor_handle: 1,
            processors: BTreeMap::new(),
        })
    }

    pub fn install_and_create_dsp_plugin(
        &self,
        plugin: &RuntimePluginInfo,
        capabilities: &[RuntimeCapabilityDescriptor],
        type_id: &str,
    ) -> Result<WasmtimeDspPlugin> {
        WasmPluginController::install_plugin(self, plugin, capabilities)?;
        self.create_dsp_plugin(&plugin.id, type_id)
    }
}
