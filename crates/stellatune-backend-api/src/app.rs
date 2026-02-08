use anyhow::Result;

use crate::player;
use crate::runtime::init_tracing;
use crate::session::{BackendSession, BackendSessionOptions};

#[derive(Default)]
pub struct BackendApp;

impl BackendApp {
    pub fn new() -> Self {
        init_tracing();
        Self
    }

    pub fn create_session(&self, options: BackendSessionOptions) -> Result<BackendSession> {
        BackendSession::from_options(options)
    }

    pub fn create_default_session(&self) -> Result<BackendSession> {
        self.create_session(BackendSessionOptions::default())
    }

    pub fn plugins_install_from_file(
        &self,
        plugins_dir: String,
        artifact_path: String,
    ) -> Result<String> {
        player::plugins_install_from_file(plugins_dir, artifact_path)
    }

    pub fn plugins_list_installed_json(&self, plugins_dir: String) -> Result<String> {
        player::plugins_list_installed_json(plugins_dir)
    }

    pub fn plugins_uninstall_by_id(&self, plugins_dir: String, plugin_id: String) -> Result<()> {
        player::plugins_uninstall_by_id(plugins_dir, plugin_id)
    }
}
