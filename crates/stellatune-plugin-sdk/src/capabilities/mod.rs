mod decoder;
mod dsp;
mod lyrics;
mod output_sink;
mod source;

pub use decoder::*;
pub use dsp::*;
pub use lyrics::*;
pub use output_sink::*;
pub use source::*;

use crate::common::ConfigUpdatePlan;
use crate::error::SdkResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbilityKind {
    Decoder,
    Source,
    Lyrics,
    OutputSink,
    Dsp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbilityDescriptor {
    pub kind: AbilityKind,
    pub type_id: &'static str,
    pub display_name: &'static str,
    pub config_schema_json: &'static str,
    pub default_config_json: &'static str,
}

pub trait ConfigStateOps {
    fn plan_config_update_json(&mut self, _new_config_json: &str) -> SdkResult<ConfigUpdatePlan> {
        Ok(ConfigUpdatePlan {
            mode: crate::common::ConfigUpdateMode::HotApply,
            reason: None,
        })
    }

    fn apply_config_update_json(&mut self, _new_config_json: &str) -> SdkResult<()> {
        Ok(())
    }

    fn export_state_json(&self) -> SdkResult<Option<String>> {
        Ok(None)
    }

    fn import_state_json(&mut self, _state_json: &str) -> SdkResult<()> {
        Ok(())
    }
}
