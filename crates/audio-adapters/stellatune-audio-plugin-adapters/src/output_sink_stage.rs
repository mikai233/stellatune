use stellatune_audio_core::pipeline::context::{AudioBlock, PipelineContext, StreamSpec};
use stellatune_audio_core::pipeline::error::PipelineError;
use stellatune_audio_core::pipeline::stages::sink::SinkStage;
use stellatune_audio_core::pipeline::stages::{Stage, StageFlow};
use stellatune_plugins::host_runtime::RuntimeOutputSinkPlugin;

use crate::output_sink_runtime::{create_output_sink_controller_and_open, write_all_frames};

const DEFAULT_WRITE_RETRY_SLEEP_MS: u64 = 2;
const DEFAULT_WRITE_STALL_TIMEOUT_MS: u64 = 250;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginOutputSinkRouteSpec {
    pub plugin_id: String,
    pub type_id: String,
    pub config_json: String,
    pub target_json: String,
}

impl PluginOutputSinkRouteSpec {
    pub fn new(
        plugin_id: String,
        type_id: String,
        config_json: String,
        target_json: String,
    ) -> Result<Self, String> {
        let plugin_id = plugin_id.trim().to_string();
        if plugin_id.is_empty() {
            return Err("plugin output sink route plugin_id must not be empty".to_string());
        }
        let type_id = type_id.trim().to_string();
        if type_id.is_empty() {
            return Err("plugin output sink route type_id must not be empty".to_string());
        }
        validate_json_payload("config_json", &config_json)?;
        validate_json_payload("target_json", &target_json)?;
        Ok(Self {
            plugin_id,
            type_id,
            config_json,
            target_json,
        })
    }
}

fn validate_json_payload(label: &str, payload: &str) -> Result<(), String> {
    serde_json::from_str::<serde_json::Value>(payload)
        .map(|_| ())
        .map_err(|error| format!("plugin output sink route {label} is not valid json: {error}"))
}

pub struct PluginOutputSinkStage {
    route: PluginOutputSinkRouteSpec,
    sink: Option<RuntimeOutputSinkPlugin>,
    prepared_spec: Option<StreamSpec>,
}

impl PluginOutputSinkStage {
    pub fn new(route: PluginOutputSinkRouteSpec) -> Self {
        Self {
            route,
            sink: None,
            prepared_spec: None,
        }
    }

    fn route_label(&self) -> String {
        format!("{}::{}", self.route.plugin_id, self.route.type_id)
    }

    fn stage_failure(&self, detail: impl Into<String>) -> PipelineError {
        PipelineError::StageFailure(format!(
            "plugin output sink {}: {}",
            self.route_label(),
            detail.into()
        ))
    }

    fn open_sink(&mut self, spec: StreamSpec) -> Result<(), PipelineError> {
        let sink = create_output_sink_controller_and_open(
            &self.route.plugin_id,
            &self.route.type_id,
            &self.route.config_json,
            &self.route.target_json,
            spec.sample_rate,
            spec.channels,
        )
        .map_err(|error| self.stage_failure(error))?;
        self.sink = Some(sink);
        self.prepared_spec = Some(spec);
        Ok(())
    }
}

impl Stage for PluginOutputSinkStage {}

impl SinkStage for PluginOutputSinkStage {
    fn prepare(
        &mut self,
        spec: StreamSpec,
        ctx: &mut PipelineContext,
    ) -> Result<(), PipelineError> {
        self.stop(ctx);
        self.open_sink(spec)
    }

    fn write(
        &mut self,
        block: &AudioBlock,
        _ctx: &mut PipelineContext,
    ) -> Result<StageFlow, PipelineError> {
        let Some(sink) = self.sink.as_mut() else {
            return Err(self.stage_failure("is not prepared"));
        };

        match write_all_frames(
            sink,
            block.channels,
            &block.samples,
            DEFAULT_WRITE_RETRY_SLEEP_MS,
            DEFAULT_WRITE_STALL_TIMEOUT_MS,
        ) {
            Ok(()) => Ok(StageFlow::Continue),
            Err(error) => Err(self.stage_failure(format!("write failed: {error}"))),
        }
    }

    fn flush(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
        if let Some(sink) = self.sink.as_mut() {
            sink.flush()
                .map_err(|e| self.stage_failure(format!("flush failed: {e}")))?;
        }
        Ok(())
    }

    fn stop(&mut self, _ctx: &mut PipelineContext) {
        if let Some(sink) = self.sink.as_mut() {
            let _ = sink.close();
        }
        self.sink = None;
        self.prepared_spec = None;
    }
}

#[cfg(test)]
mod tests {
    use super::PluginOutputSinkRouteSpec;

    #[test]
    fn route_spec_rejects_invalid_json_payloads() {
        let invalid_config = PluginOutputSinkRouteSpec::new(
            "plugin".to_string(),
            "sink".to_string(),
            "not-json".to_string(),
            "{}".to_string(),
        );
        assert!(invalid_config.is_err());

        let invalid_target = PluginOutputSinkRouteSpec::new(
            "plugin".to_string(),
            "sink".to_string(),
            "{}".to_string(),
            "not-json".to_string(),
        );
        assert!(invalid_target.is_err());
    }

    #[test]
    fn route_spec_accepts_valid_json_payloads() {
        let route = PluginOutputSinkRouteSpec::new(
            "plugin".to_string(),
            "sink".to_string(),
            r#"{"foo":1}"#.to_string(),
            r#"{"bar":"x"}"#.to_string(),
        )
        .expect("route spec should be accepted");

        assert_eq!(route.plugin_id, "plugin");
        assert_eq!(route.type_id, "sink");
    }
}
