#[macro_export]
macro_rules! export_lyrics_component {
    (
        plugin_type: $plugin_ty:ty,
        create: $create:path $(,)?
    ) => {
        mod __st_lyrics_component_export {
            use super::*;
            use $crate::__private::parking_lot::{Mutex, MutexGuard};
            use std::sync::OnceLock;
            use $crate::__private::stellatune_wasm_guest_bindings_lyrics as __st_bindings;

            type __StPlugin = $plugin_ty;
            type __StPluginError =
                __st_bindings::exports::stellatune::plugin::lyrics::PluginError;
            type __StDisableReason =
                __st_bindings::exports::stellatune::plugin::lifecycle::DisableReason;
            type __StConfigUpdateMode =
                __st_bindings::stellatune::plugin::common::ConfigUpdateMode;
            type __StConfigUpdatePlan =
                __st_bindings::exports::stellatune::plugin::lyrics::ConfigUpdatePlan;
            type __StLyricCandidate =
                __st_bindings::exports::stellatune::plugin::lyrics::LyricCandidate;

            static __ST_PLUGIN: OnceLock<Mutex<__StPlugin>> = OnceLock::new();

            struct __StRoot;
            struct __StProvider {
                inner: Mutex<<__StPlugin as $crate::LyricsPlugin>::Provider>,
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

            fn __map_lyric_candidate(item: $crate::common::LyricCandidate) -> __StLyricCandidate {
                __StLyricCandidate {
                    id: item.id,
                    title: item.title,
                    artist: item.artist,
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

            impl __st_bindings::exports::stellatune::plugin::lyrics::Guest for __StRoot {
                type Provider = __StProvider;

                fn create(
                ) -> Result<__st_bindings::exports::stellatune::plugin::lyrics::Provider, __StPluginError>
                {
                    let mut plugin = __plugin_guard()?;
                    let provider = plugin.create_provider().map_err(__map_error)?;
                    Ok(__st_bindings::exports::stellatune::plugin::lyrics::Provider::new(
                        __StProvider {
                            inner: Mutex::new(provider),
                        },
                    ))
                }
            }

            impl __st_bindings::exports::stellatune::plugin::lyrics::GuestProvider for __StProvider {
                fn search(&self, keyword: String) -> Result<Vec<__StLyricCandidate>, __StPluginError> {
                    let mut provider = self.inner.lock();
                    let items = provider.search(keyword.as_str()).map_err(__map_error)?;
                    Ok(items.into_iter().map(__map_lyric_candidate).collect())
                }

                fn fetch(&self, id: String) -> Result<String, __StPluginError> {
                    let mut provider = self.inner.lock();
                    provider.fetch(id.as_str()).map_err(__map_error)
                }

                fn plan_config_update_json(
                    &self,
                    new_config_json: String,
                ) -> Result<__StConfigUpdatePlan, __StPluginError> {
                    let mut provider = self.inner.lock();
                    provider
                        .plan_config_update_json(new_config_json.as_str())
                        .map(__map_config_update_plan)
                        .map_err(__map_error)
                }

                fn apply_config_update_json(
                    &self,
                    new_config_json: String,
                ) -> Result<(), __StPluginError> {
                    let mut provider = self.inner.lock();
                    provider
                        .apply_config_update_json(new_config_json.as_str())
                        .map_err(__map_error)
                }

                fn export_state_json(&self) -> Result<Option<String>, __StPluginError> {
                    let provider = self.inner.lock();
                    provider.export_state_json().map_err(__map_error)
                }

                fn import_state_json(&self, state_json: String) -> Result<(), __StPluginError> {
                    let mut provider = self.inner.lock();
                    provider
                        .import_state_json(state_json.as_str())
                        .map_err(__map_error)
                }

                fn close(&self) {
                    let mut provider = self.inner.lock();
                    let _ = provider.close();
                }
            }

            __st_bindings::export!(__StRoot with_types_in __st_bindings);
        }
    };
}
