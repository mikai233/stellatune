use std::sync::mpsc;

use crate::error::Result;
use wasmtime::Store;

use stellatune_wasm_host_bindings::generated as host_bindings;

use host_bindings::output_sink_plugin::OutputSinkPlugin as OutputSinkBinding;
use host_bindings::output_sink_plugin::exports::stellatune::plugin::output_sink as output_sink_exports;
use host_bindings::output_sink_plugin::stellatune::plugin::common as output_sink_common;
use host_bindings::output_sink_plugin::stellatune::plugin::hot_path as output_sink_hot_path;

use crate::executor::plugin_cell::{PluginCell, PluginCellState};
use crate::executor::stores::output_sink::OutputSinkStoreData;
use crate::executor::{
    WasmPluginController, WasmtimePluginController, WorldKind, classify_world,
    map_disable_reason_output_sink,
};
use crate::manifest::AbilityKind;
use crate::runtime::model::{
    PluginDisableReason, RuntimeAudioSpec, RuntimeBufferLayout, RuntimeCapabilityDescriptor,
    RuntimeCoreModuleSpec, RuntimeHotPathRole, RuntimeNegotiatedSpec, RuntimeOutputSinkStatus,
    RuntimePluginDirective, RuntimePluginInfo, RuntimeSampleFormat,
};

use crate::executor::plugin_instance::common::reconcile_with;

pub trait OutputSinkPluginApi {
    fn list_targets_json(&mut self) -> Result<String>;
    fn negotiate_spec_json(
        &mut self,
        target_json: &str,
        desired: RuntimeAudioSpec,
    ) -> Result<RuntimeNegotiatedSpec>;
    fn describe_hot_path(
        &mut self,
        spec: RuntimeAudioSpec,
    ) -> Result<Option<RuntimeCoreModuleSpec>>;
    fn open_json(&mut self, target_json: &str, spec: RuntimeAudioSpec) -> Result<()>;
    fn write_interleaved_f32(&mut self, channels: u16, interleaved_f32le: Vec<u8>) -> Result<u32>;
    fn query_status(&mut self) -> Result<RuntimeOutputSinkStatus>;
    fn flush(&mut self) -> Result<()>;
    fn reset(&mut self) -> Result<()>;
    fn plan_config_update_json(&mut self, config_json: &str) -> Result<(String, Option<String>)>;
    fn apply_config_update_json(&mut self, config_json: &str) -> Result<()>;
    fn export_state_json(&mut self) -> Result<Option<String>>;
    fn import_state_json(&mut self, state_json: &str) -> Result<()>;
    fn close(&mut self) -> Result<()>;
}

pub struct WasmtimeOutputSinkPlugin {
    plugin_id: String,
    component: PluginCell<Store<OutputSinkStoreData>, OutputSinkBinding>,
    session: Option<wasmtime::component::ResourceAny>,
}

impl WasmtimeOutputSinkPlugin {
    pub fn plugin_id(&self) -> &str {
        &self.plugin_id
    }

    fn output_sink_api(&self) -> output_sink_exports::Guest {
        self.component
            .plugin
            .stellatune_plugin_output_sink()
            .clone()
    }

    fn ensure_session(&mut self) -> Result<wasmtime::component::ResourceAny> {
        if let Some(session) = self.session {
            return Ok(session);
        }
        let output = self.output_sink_api();
        let session = output
            .call_create(&mut self.component.store)?
            .map_err(|error| crate::op_error!("output-sink.create plugin error: {error:?}"))?;
        self.session = Some(session);
        self.session
            .ok_or_else(|| crate::op_error!("output-sink session handle missing after create"))
    }

    fn reconcile_runtime(&mut self) -> Result<()> {
        let session = self.session;
        let mut rebuilt = false;
        let mut destroyed = false;
        reconcile_with(
            &mut self.component,
            |store, plugin, config_json| {
                let output = plugin.stellatune_plugin_output_sink();
                if let Some(session_ref) = session {
                    let plan = output
                        .session()
                        .call_plan_config_update_json(&mut *store, session_ref, config_json)?
                        .map_err(|error| crate::op_error!("output-sink.session.plan-config-update-json plugin error: {error:?}"))?;
                    match plan.mode {
                        output_sink_common::ConfigUpdateMode::HotApply => {
                            output
                                .session()
                                .call_apply_config_update_json(
                                    &mut *store,
                                    session_ref,
                                    config_json,
                                )?
                                .map_err(|error| crate::op_error!("output-sink.session.apply-config-update-json plugin error: {error:?}"))?;
                        },
                        output_sink_common::ConfigUpdateMode::Recreate => {
                            return Err(crate::op_error!(
                                "output-sink requested recreate for config update"
                            ));
                        },
                        output_sink_common::ConfigUpdateMode::Reject => {
                            return Err(crate::op_error!(
                                "output-sink rejected config update: {}",
                                plan.reason.unwrap_or_else(|| "unknown".to_string())
                            ));
                        },
                    }
                }
                Ok(())
            },
            |store, plugin| {
                let output = plugin.stellatune_plugin_output_sink();
                if let Some(session_ref) = session {
                    let _ = output.session().call_close(&mut *store, session_ref);
                    let _ = session_ref.resource_drop(&mut *store);
                }
                let disable = plugin
                    .stellatune_plugin_lifecycle()
                    .call_on_disable(
                        &mut *store,
                        map_disable_reason_output_sink(PluginDisableReason::Reload),
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
                let output = plugin.stellatune_plugin_output_sink();
                if let Some(session_ref) = session {
                    let _ = output.session().call_close(&mut *store, session_ref);
                    let _ = session_ref.resource_drop(&mut *store);
                }
                let disable = plugin
                    .stellatune_plugin_lifecycle()
                    .call_on_disable(&mut *store, map_disable_reason_output_sink(reason))
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
            self.session = None;
        }
        Ok(())
    }
}

impl OutputSinkPluginApi for WasmtimeOutputSinkPlugin {
    fn list_targets_json(&mut self) -> Result<String> {
        self.reconcile_runtime()?;
        let session = self.ensure_session()?;
        let output = self.output_sink_api();
        output
            .session()
            .call_list_targets_json(&mut self.component.store, session)?
            .map_err(|error| {
                crate::op_error!("output-sink.session.list-targets-json plugin error: {error:?}")
            })
    }

    fn negotiate_spec_json(
        &mut self,
        target_json: &str,
        desired: RuntimeAudioSpec,
    ) -> Result<RuntimeNegotiatedSpec> {
        self.reconcile_runtime()?;
        let session = self.ensure_session()?;
        let output = self.output_sink_api();
        let negotiated = output
            .session()
            .call_negotiate_spec_json(
                &mut self.component.store,
                session,
                target_json,
                output_sink_exports::AudioSpec {
                    sample_rate: desired.sample_rate,
                    channels: desired.channels,
                },
            )?
            .map_err(|error| {
                crate::op_error!("output-sink.session.negotiate-spec-json plugin error: {error:?}")
            })?;
        Ok(RuntimeNegotiatedSpec {
            spec: RuntimeAudioSpec {
                sample_rate: negotiated.spec.sample_rate,
                channels: negotiated.spec.channels,
            },
            preferred_chunk_frames: negotiated.preferred_chunk_frames,
            prefer_track_rate: negotiated.prefer_track_rate,
        })
    }

    fn describe_hot_path(
        &mut self,
        spec: RuntimeAudioSpec,
    ) -> Result<Option<RuntimeCoreModuleSpec>> {
        self.reconcile_runtime()?;
        let session = self.ensure_session()?;
        let output = self.output_sink_api();
        let maybe_spec = output
            .session()
            .call_describe_hot_path(
                &mut self.component.store,
                session,
                output_sink_exports::AudioSpec {
                    sample_rate: spec.sample_rate,
                    channels: spec.channels,
                },
            )?
            .map_err(|error| {
                crate::op_error!("output-sink.session.describe-hot-path plugin error: {error:?}")
            })?;
        Ok(maybe_spec.map(|spec| RuntimeCoreModuleSpec {
            role: match spec.role {
                output_sink_hot_path::Role::DspTransform => RuntimeHotPathRole::DspTransform,
                output_sink_hot_path::Role::OutputSink => RuntimeHotPathRole::OutputSink,
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
                    output_sink_hot_path::SampleFormat::F32le => RuntimeSampleFormat::F32Le,
                    output_sink_hot_path::SampleFormat::I16le => RuntimeSampleFormat::I16Le,
                    output_sink_hot_path::SampleFormat::I32le => RuntimeSampleFormat::I32Le,
                },
                interleaved: spec.buffer.interleaved,
            },
        }))
    }

    fn open_json(&mut self, target_json: &str, spec: RuntimeAudioSpec) -> Result<()> {
        self.reconcile_runtime()?;
        let session = self.ensure_session()?;
        let output = self.output_sink_api();
        output
            .session()
            .call_open_json(
                &mut self.component.store,
                session,
                target_json,
                output_sink_exports::AudioSpec {
                    sample_rate: spec.sample_rate,
                    channels: spec.channels,
                },
            )?
            .map_err(|error| {
                crate::op_error!("output-sink.session.open-json plugin error: {error:?}")
            })?;
        Ok(())
    }

    fn write_interleaved_f32(&mut self, channels: u16, interleaved_f32le: Vec<u8>) -> Result<u32> {
        self.reconcile_runtime()?;
        let session = self.ensure_session()?;
        let output = self.output_sink_api();
        output
            .session()
            .call_write_interleaved_f32(
                &mut self.component.store,
                session,
                channels,
                &interleaved_f32le,
            )?
            .map_err(|error| {
                crate::op_error!(
                    "output-sink.session.write-interleaved-f32 plugin error: {error:?}"
                )
            })
    }

    fn query_status(&mut self) -> Result<RuntimeOutputSinkStatus> {
        self.reconcile_runtime()?;
        let session = self.ensure_session()?;
        let output = self.output_sink_api();
        let status = output
            .session()
            .call_query_status(&mut self.component.store, session)?
            .map_err(|error| {
                crate::op_error!("output-sink.session.query-status plugin error: {error:?}")
            })?;
        Ok(RuntimeOutputSinkStatus {
            queued_samples: status.queued_samples,
            running: status.running,
        })
    }

    fn flush(&mut self) -> Result<()> {
        self.reconcile_runtime()?;
        let session = self.ensure_session()?;
        let output = self.output_sink_api();
        output
            .session()
            .call_flush(&mut self.component.store, session)?
            .map_err(|error| {
                crate::op_error!("output-sink.session.flush plugin error: {error:?}")
            })?;
        Ok(())
    }

    fn reset(&mut self) -> Result<()> {
        self.reconcile_runtime()?;
        let session = self.ensure_session()?;
        let output = self.output_sink_api();
        output
            .session()
            .call_reset(&mut self.component.store, session)?
            .map_err(|error| {
                crate::op_error!("output-sink.session.reset plugin error: {error:?}")
            })?;
        Ok(())
    }

    fn plan_config_update_json(&mut self, config_json: &str) -> Result<(String, Option<String>)> {
        self.reconcile_runtime()?;
        let session = self.ensure_session()?;
        let output = self.output_sink_api();
        let plan = output
            .session()
            .call_plan_config_update_json(&mut self.component.store, session, config_json)?
            .map_err(|error| {
                crate::op_error!(
                    "output-sink.session.plan-config-update-json plugin error: {error:?}"
                )
            })?;
        Ok((format!("{:?}", plan.mode), plan.reason))
    }

    fn apply_config_update_json(&mut self, config_json: &str) -> Result<()> {
        self.reconcile_runtime()?;
        let session = self.ensure_session()?;
        let output = self.output_sink_api();
        output
            .session()
            .call_apply_config_update_json(&mut self.component.store, session, config_json)?
            .map_err(|error| {
                crate::op_error!(
                    "output-sink.session.apply-config-update-json plugin error: {error:?}"
                )
            })?;
        Ok(())
    }

    fn export_state_json(&mut self) -> Result<Option<String>> {
        self.reconcile_runtime()?;
        let session = self.ensure_session()?;
        let output = self.output_sink_api();
        output
            .session()
            .call_export_state_json(&mut self.component.store, session)?
            .map_err(|error| {
                crate::op_error!("output-sink.session.export-state-json plugin error: {error:?}")
            })
    }

    fn import_state_json(&mut self, state_json: &str) -> Result<()> {
        self.reconcile_runtime()?;
        let session = self.ensure_session()?;
        let output = self.output_sink_api();
        output
            .session()
            .call_import_state_json(&mut self.component.store, session, state_json)?
            .map_err(|error| {
                crate::op_error!("output-sink.session.import-state-json plugin error: {error:?}")
            })?;
        Ok(())
    }

    fn close(&mut self) -> Result<()> {
        let Some(session) = self.session.take() else {
            return Ok(());
        };
        let output = self.output_sink_api();
        let _ = output
            .session()
            .call_close(&mut self.component.store, session);
        let _ = session.resource_drop(&mut self.component.store);
        Ok(())
    }
}

impl Drop for WasmtimeOutputSinkPlugin {
    fn drop(&mut self) {
        let _ = self.close();
        if self.component.state() != PluginCellState::Destroyed {
            let _ = self
                .component
                .plugin
                .stellatune_plugin_lifecycle()
                .call_on_disable(
                    &mut self.component.store,
                    map_disable_reason_output_sink(PluginDisableReason::HostDisable),
                );
        }
    }
}

impl WasmtimePluginController {
    pub fn create_output_sink_plugin(
        &self,
        plugin_id: &str,
        type_id: &str,
    ) -> Result<WasmtimeOutputSinkPlugin> {
        let (plugin, capability) =
            self.resolve_capability(plugin_id, AbilityKind::OutputSink, type_id)?;
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
            WorldKind::OutputSink => {
                self.instantiate_output_sink_component(&plugin.root_dir, &component, rx)?
            },
            _ => {
                return Err(crate::op_error!(
                    "capability world `{}` is not an output-sink world",
                    capability.world
                ));
            },
        };
        self.register_directive_sender(plugin_id, tx)?;

        Ok(WasmtimeOutputSinkPlugin {
            plugin_id: plugin_id.to_string(),
            component,
            session: None,
        })
    }

    pub fn install_and_create_output_sink_plugin(
        &self,
        plugin: &RuntimePluginInfo,
        capabilities: &[RuntimeCapabilityDescriptor],
        type_id: &str,
    ) -> Result<WasmtimeOutputSinkPlugin> {
        WasmPluginController::install_plugin(self, plugin, capabilities)?;
        self.create_output_sink_plugin(&plugin.id, type_id)
    }
}
