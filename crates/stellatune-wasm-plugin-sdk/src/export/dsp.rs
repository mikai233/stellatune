#[macro_export]
macro_rules! export_dsp_component {
    (
        plugin_type: $plugin_ty:ty,
        create: $create:path $(,)?
    ) => {
        mod __st_dsp_component_export {
            use super::*;
            use $crate::__private::parking_lot::{Mutex, MutexGuard};
            use std::sync::OnceLock;
            use $crate::__private::stellatune_wasm_guest_bindings_dsp as __st_bindings;

            type __StPlugin = $plugin_ty;
            type __StPluginError = __st_bindings::exports::stellatune::plugin::dsp::PluginError;
            type __StDisableReason =
                __st_bindings::exports::stellatune::plugin::lifecycle::DisableReason;
            type __StAudioSpec = __st_bindings::exports::stellatune::plugin::dsp::AudioSpec;
            type __StConfigUpdateMode =
                __st_bindings::stellatune::plugin::common::ConfigUpdateMode;
            type __StConfigUpdatePlan =
                __st_bindings::exports::stellatune::plugin::dsp::ConfigUpdatePlan;
            type __StCoreModuleSpec = __st_bindings::exports::stellatune::plugin::dsp::CoreModuleSpec;
            type __StBufferLayout = __st_bindings::stellatune::plugin::hot_path::BufferLayout;
            type __StRole = __st_bindings::stellatune::plugin::hot_path::Role;
            type __StSampleFormat = __st_bindings::stellatune::plugin::hot_path::SampleFormat;

            static __ST_PLUGIN: OnceLock<Mutex<__StPlugin>> = OnceLock::new();

            struct __StRoot;
            struct __StProcessor {
                inner: Mutex<<__StPlugin as $crate::DspPlugin>::Processor>,
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

            fn __map_audio_spec(spec: $crate::common::AudioSpec) -> __StAudioSpec {
                __StAudioSpec {
                    sample_rate: spec.sample_rate,
                    channels: spec.channels,
                }
            }

            fn __map_role(role: $crate::common::HotPathRole) -> __StRole {
                match role {
                    $crate::common::HotPathRole::DspTransform => __StRole::DspTransform,
                    $crate::common::HotPathRole::OutputSink => __StRole::OutputSink,
                }
            }

            fn __map_sample_format(format: $crate::common::SampleFormat) -> __StSampleFormat {
                match format {
                    $crate::common::SampleFormat::F32Le => __StSampleFormat::F32le,
                    $crate::common::SampleFormat::I16Le => __StSampleFormat::I16le,
                    $crate::common::SampleFormat::I32Le => __StSampleFormat::I32le,
                }
            }

            fn __map_buffer_layout(layout: $crate::common::BufferLayout) -> __StBufferLayout {
                __StBufferLayout {
                    in_offset: layout.in_offset,
                    out_offset: layout.out_offset,
                    max_frames: layout.max_frames,
                    channels: layout.channels,
                    sample_format: __map_sample_format(layout.sample_format),
                    interleaved: layout.interleaved,
                }
            }

            fn __map_core_module_spec(spec: $crate::common::CoreModuleSpec) -> __StCoreModuleSpec {
                __StCoreModuleSpec {
                    role: __map_role(spec.role),
                    wasm_rel_path: spec.wasm_rel_path,
                    abi_version: spec.abi_version,
                    memory_export: spec.memory_export,
                    init_export: spec.init_export,
                    process_export: spec.process_export,
                    reset_export: spec.reset_export,
                    drop_export: spec.drop_export,
                    buffer: __map_buffer_layout(spec.buffer),
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

            impl __st_bindings::exports::stellatune::plugin::dsp::Guest for __StRoot {
                type Processor = __StProcessor;

                fn create(
                    spec: __StAudioSpec,
                ) -> Result<__st_bindings::exports::stellatune::plugin::dsp::Processor, __StPluginError>
                {
                    let mut plugin = __plugin_guard()?;
                    let processor = plugin
                        .create_processor($crate::common::AudioSpec {
                            sample_rate: spec.sample_rate,
                            channels: spec.channels,
                        })
                        .map_err(__map_error)?;
                    Ok(__st_bindings::exports::stellatune::plugin::dsp::Processor::new(
                        __StProcessor {
                            inner: Mutex::new(processor),
                        },
                    ))
                }
            }

            impl __st_bindings::exports::stellatune::plugin::dsp::GuestProcessor for __StProcessor {
                fn describe_hot_path(
                    &self,
                    spec: __StAudioSpec,
                ) -> Result<Option<__StCoreModuleSpec>, __StPluginError> {
                    let mut processor = self.inner.lock();
                    processor
                        .describe_hot_path($crate::common::AudioSpec {
                            sample_rate: spec.sample_rate,
                            channels: spec.channels,
                        })
                        .map(|v| v.map(__map_core_module_spec))
                        .map_err(__map_error)
                }

                fn process_interleaved_f32(
                    &self,
                    channels: u16,
                    interleaved_f32le: Vec<u8>,
                ) -> Result<Vec<u8>, __StPluginError> {
                    let mut processor = self.inner.lock();
                    processor
                        .process_interleaved_f32(channels, interleaved_f32le.as_slice())
                        .map_err(__map_error)
                }

                fn supported_layouts(&self) -> u32 {
                    let processor = self.inner.lock();
                    processor.supported_layouts()
                }

                fn output_channels(&self) -> u16 {
                    let processor = self.inner.lock();
                    processor.output_channels()
                }

                fn plan_config_update_json(
                    &self,
                    new_config_json: String,
                ) -> Result<__StConfigUpdatePlan, __StPluginError> {
                    let mut processor = self.inner.lock();
                    processor
                        .plan_config_update_json(new_config_json.as_str())
                        .map(__map_config_update_plan)
                        .map_err(__map_error)
                }

                fn apply_config_update_json(
                    &self,
                    new_config_json: String,
                ) -> Result<(), __StPluginError> {
                    let mut processor = self.inner.lock();
                    processor
                        .apply_config_update_json(new_config_json.as_str())
                        .map_err(__map_error)
                }

                fn export_state_json(&self) -> Result<Option<String>, __StPluginError> {
                    let processor = self.inner.lock();
                    processor.export_state_json().map_err(__map_error)
                }

                fn import_state_json(&self, state_json: String) -> Result<(), __StPluginError> {
                    let mut processor = self.inner.lock();
                    processor
                        .import_state_json(state_json.as_str())
                        .map_err(__map_error)
                }

                fn close(&self) {
                    let mut processor = self.inner.lock();
                    let _ = processor.close();
                }
            }

            __st_bindings::export!(__StRoot with_types_in __st_bindings);
        }
    };
}
