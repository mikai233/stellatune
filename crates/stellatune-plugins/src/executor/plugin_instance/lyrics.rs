use std::sync::mpsc;

use crate::error::Result;
use wasmtime::Store;

use stellatune_host_bindings::generated as host_bindings;

use host_bindings::lyrics_plugin::LyricsPlugin as LyricsBinding;
use host_bindings::lyrics_plugin::stellatune::plugin::common as lyrics_common;

use crate::executor::plugin_cell::{PluginCell, PluginCellState};
use crate::executor::stores::lyrics::LyricsStoreData;
use crate::executor::{
    WasmPluginController, WasmtimePluginController, WorldKind, call_lyrics_on_disable,
    call_lyrics_on_enable, classify_world, map_disable_reason_lyrics,
};
use crate::manifest::AbilityKind;
use crate::runtime::model::{
    PluginDisableReason, RuntimeCapabilityDescriptor, RuntimeLyricCandidate,
    RuntimePluginDirective, RuntimePluginInfo,
};

use crate::executor::plugin_instance::common::{map_lyrics_plugin_error, reconcile_with};

pub trait LyricsPluginApi {
    fn search(&mut self, keyword: &str) -> Result<Vec<RuntimeLyricCandidate>>;
    fn fetch(&mut self, lyric_id: &str) -> Result<String>;
}

pub struct WasmtimeLyricsPlugin {
    plugin_id: String,
    component: PluginCell<Store<LyricsStoreData>, LyricsBinding>,
}

impl WasmtimeLyricsPlugin {
    pub fn plugin_id(&self) -> &str {
        &self.plugin_id
    }

    fn reconcile_runtime(&mut self) -> Result<()> {
        reconcile_with(
            &mut self.component,
            |store, plugin, config_json| {
                let lyrics = plugin.stellatune_plugin_lyrics();
                let provider =
                    map_lyrics_plugin_error(lyrics.call_create(&mut *store)?, "lyrics.create")?;
                let plan = map_lyrics_plugin_error(
                    lyrics.provider().call_plan_config_update_json(
                        &mut *store,
                        provider,
                        config_json,
                    )?,
                    "lyrics.provider.plan-config-update-json",
                )?;
                match plan.mode {
                    lyrics_common::ConfigUpdateMode::HotApply => {
                        map_lyrics_plugin_error(
                            lyrics.provider().call_apply_config_update_json(
                                &mut *store,
                                provider,
                                config_json,
                            )?,
                            "lyrics.provider.apply-config-update-json",
                        )?;
                    },
                    lyrics_common::ConfigUpdateMode::Recreate => {
                        return Err(crate::op_error!(
                            "lyrics provider requested recreate for config update"
                        ));
                    },
                    lyrics_common::ConfigUpdateMode::Reject => {
                        return Err(crate::op_error!(
                            "lyrics provider rejected config update: {}",
                            plan.reason.unwrap_or_else(|| "unknown".to_string())
                        ));
                    },
                }
                let _ = lyrics.provider().call_close(&mut *store, provider);
                let _ = provider.resource_drop(&mut *store);
                Ok(())
            },
            |store, plugin| {
                call_lyrics_on_disable(
                    plugin,
                    store,
                    map_disable_reason_lyrics(PluginDisableReason::Reload),
                )?;
                call_lyrics_on_enable(plugin, store)?;
                Ok(())
            },
            |store, plugin, reason| {
                call_lyrics_on_disable(plugin, store, map_disable_reason_lyrics(reason))?;
                Ok(())
            },
        )
    }
}

impl LyricsPluginApi for WasmtimeLyricsPlugin {
    fn search(&mut self, keyword: &str) -> Result<Vec<RuntimeLyricCandidate>> {
        let keyword = keyword.trim();
        if keyword.is_empty() {
            return Ok(Vec::new());
        }

        self.reconcile_runtime()?;
        let lyrics = self.component.plugin.stellatune_plugin_lyrics();
        let provider = map_lyrics_plugin_error(
            lyrics.call_create(&mut self.component.store)?,
            "lyrics.create",
        )?;
        let out = map_lyrics_plugin_error(
            lyrics
                .provider()
                .call_search(&mut self.component.store, provider, keyword)?,
            "lyrics.provider.search",
        )?
        .into_iter()
        .map(|item| RuntimeLyricCandidate {
            id: item.id,
            title: item.title,
            artist: item.artist,
        })
        .collect::<Vec<_>>();
        let _ = lyrics
            .provider()
            .call_close(&mut self.component.store, provider);
        let _ = provider.resource_drop(&mut self.component.store);
        Ok(out)
    }

    fn fetch(&mut self, lyric_id: &str) -> Result<String> {
        let lyric_id = lyric_id.trim();
        if lyric_id.is_empty() {
            return Err(crate::op_error!("lyric_id is empty"));
        }

        self.reconcile_runtime()?;
        let lyrics = self.component.plugin.stellatune_plugin_lyrics();
        let provider = map_lyrics_plugin_error(
            lyrics.call_create(&mut self.component.store)?,
            "lyrics.create",
        )?;
        let out = map_lyrics_plugin_error(
            lyrics
                .provider()
                .call_fetch(&mut self.component.store, provider, lyric_id)?,
            "lyrics.provider.fetch",
        )?;
        let _ = lyrics
            .provider()
            .call_close(&mut self.component.store, provider);
        let _ = provider.resource_drop(&mut self.component.store);
        Ok(out)
    }
}

impl Drop for WasmtimeLyricsPlugin {
    fn drop(&mut self) {
        if self.component.state() != PluginCellState::Destroyed {
            let _ = call_lyrics_on_disable(
                &self.component.plugin,
                &mut self.component.store,
                map_disable_reason_lyrics(PluginDisableReason::HostDisable),
            );
        }
    }
}

impl WasmtimePluginController {
    pub fn create_lyrics_plugin(
        &self,
        plugin_id: &str,
        type_id: &str,
    ) -> Result<WasmtimeLyricsPlugin> {
        let (plugin, capability) =
            self.resolve_capability(plugin_id, AbilityKind::Lyrics, type_id)?;
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
            WorldKind::Lyrics => {
                self.instantiate_lyrics_component(plugin_id, &plugin.root_dir, &component, rx)?
            },
            _ => {
                return Err(crate::op_error!(
                    "capability world `{}` is not a lyrics world",
                    capability.world
                ));
            },
        };

        self.register_directive_sender(plugin_id, tx)?;

        Ok(WasmtimeLyricsPlugin {
            plugin_id: plugin_id.to_string(),
            component,
        })
    }

    pub fn install_and_create_lyrics_plugin(
        &self,
        plugin: &RuntimePluginInfo,
        capabilities: &[RuntimeCapabilityDescriptor],
        type_id: &str,
    ) -> Result<WasmtimeLyricsPlugin> {
        WasmPluginController::install_plugin(self, plugin, capabilities)?;
        self.create_lyrics_plugin(&plugin.id, type_id)
    }
}
