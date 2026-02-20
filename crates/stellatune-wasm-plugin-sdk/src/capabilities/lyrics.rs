use crate::capabilities::{AbilityDescriptor, AbilityKind, ConfigStateOps};
use crate::common::LyricCandidate;
use crate::error::SdkResult;
use crate::lifecycle::PluginLifecycle;

pub trait LyricsProvider: ConfigStateOps + Send {
    fn search(&mut self, keyword: &str) -> SdkResult<Vec<LyricCandidate>>;
    fn fetch(&mut self, id: &str) -> SdkResult<String>;
    fn close(&mut self) -> SdkResult<()> {
        Ok(())
    }
}

pub trait LyricsPlugin: PluginLifecycle + Send + 'static {
    type Provider: LyricsProvider;

    const TYPE_ID: &'static str;
    const DISPLAY_NAME: &'static str;
    const CONFIG_SCHEMA_JSON: &'static str = "{}";
    const DEFAULT_CONFIG_JSON: &'static str = "{}";

    fn descriptor() -> AbilityDescriptor {
        AbilityDescriptor {
            kind: AbilityKind::Lyrics,
            type_id: Self::TYPE_ID,
            display_name: Self::DISPLAY_NAME,
            config_schema_json: Self::CONFIG_SCHEMA_JSON,
            default_config_json: Self::DEFAULT_CONFIG_JSON,
        }
    }

    fn create_provider(&mut self) -> SdkResult<Self::Provider>;
}
