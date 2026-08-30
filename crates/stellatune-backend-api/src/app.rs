use anyhow::Result;

use crate::player;
use crate::plugin_ui_gateway::{
    PluginUiGatewayHandle, PluginUiGatewayOptions, start_plugin_ui_gateway,
};
use crate::runtime::init_tracing;
use crate::session::{BackendSession, BackendSessionOptions};

#[derive(Default)]
pub struct BackendApp;

impl BackendApp {
    pub fn new() -> Self {
        init_tracing();
        Self
    }

    pub async fn create_session(&self, options: BackendSessionOptions) -> Result<BackendSession> {
        BackendSession::from_options(options).await
    }

    pub async fn create_default_session(&self) -> Result<BackendSession> {
        self.create_session(BackendSessionOptions::default()).await
    }

    pub async fn plugins_install_from_file(
        &self,
        plugins_dir: String,
        artifact_path: String,
    ) -> Result<String> {
        player::plugins_install_from_file(plugins_dir, artifact_path).await
    }

    pub fn plugins_list_installed_json(&self, plugins_dir: String) -> Result<String> {
        player::plugins_list_installed_json(plugins_dir)
    }

    pub async fn plugins_uninstall_by_id(
        &self,
        plugins_dir: String,
        plugin_id: String,
    ) -> Result<()> {
        player::plugins_uninstall_by_id(plugins_dir, plugin_id).await
    }

    pub async fn plugin_ui_gateway_start(
        &self,
        options: PluginUiGatewayOptions,
    ) -> Result<PluginUiGatewayHandle> {
        start_plugin_ui_gateway(options).await
    }
}
