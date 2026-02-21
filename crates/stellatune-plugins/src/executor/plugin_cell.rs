use std::sync::mpsc::Receiver;

use crate::error::Result;

use crate::runtime::model::{PluginDisableReason, RuntimePluginDirective};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginCellState {
    Active,
    RebuildPending,
    DestroyPending,
    Destroyed,
}

pub struct PluginCell<TStore, TPlugin> {
    rx: Receiver<RuntimePluginDirective>,
    pub store: TStore,
    pub plugin: TPlugin,
    state: PluginCellState,
    pending_config: Option<String>,
    pending_destroy_reason: Option<PluginDisableReason>,
}

impl<TStore, TPlugin> PluginCell<TStore, TPlugin> {
    pub fn new(store: TStore, plugin: TPlugin, rx: Receiver<RuntimePluginDirective>) -> Self {
        Self {
            rx,
            store,
            plugin,
            state: PluginCellState::Active,
            pending_config: None,
            pending_destroy_reason: None,
        }
    }

    pub fn state(&self) -> PluginCellState {
        self.state
    }

    fn poll_directives(&mut self) {
        while let Ok(directive) = self.rx.try_recv() {
            match directive {
                RuntimePluginDirective::Destroy { reason } => {
                    self.state = PluginCellState::DestroyPending;
                    self.pending_config = None;
                    self.pending_destroy_reason = Some(reason);
                },
                RuntimePluginDirective::Rebuild => {
                    if self.state != PluginCellState::DestroyPending {
                        self.state = PluginCellState::RebuildPending;
                    }
                },
                RuntimePluginDirective::UpdateConfig { config_json } => {
                    if self.state == PluginCellState::Active
                        || self.state == PluginCellState::RebuildPending
                    {
                        self.pending_config = Some(config_json);
                    }
                },
            }
        }
    }

    pub fn reconcile<FUpdate, FRebuild, FDestroy>(
        &mut self,
        mut update: FUpdate,
        mut rebuild: FRebuild,
        mut destroy: FDestroy,
    ) -> Result<()>
    where
        FUpdate: FnMut(&mut TStore, &mut TPlugin, &str) -> Result<()>,
        FRebuild: FnMut(&mut TStore, &mut TPlugin) -> Result<()>,
        FDestroy: FnMut(&mut TStore, &mut TPlugin, PluginDisableReason) -> Result<()>,
    {
        self.poll_directives();

        if self.state == PluginCellState::DestroyPending {
            let reason = self
                .pending_destroy_reason
                .take()
                .unwrap_or(PluginDisableReason::HostDisable);
            destroy(&mut self.store, &mut self.plugin, reason)?;
            self.state = PluginCellState::Destroyed;
            self.pending_config = None;
            return Ok(());
        }

        if self.state == PluginCellState::RebuildPending {
            rebuild(&mut self.store, &mut self.plugin)?;
            self.state = PluginCellState::Active;
        }

        if let Some(config_json) = self.pending_config.take() {
            update(&mut self.store, &mut self.plugin, &config_json)?;
        }

        Ok(())
    }
}
