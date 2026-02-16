use serde::Deserialize;

pub const DSP_CHAIN_SCHEMA_ID: &str = "stellatune.audio.dsp.chain/v1";
pub const DSP_CHAIN_SCHEMA_REVISION: u64 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DspChainStage {
    PreMix,
    PostMix,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltinDspChainEntry {
    pub stage: DspChainStage,
    pub plugin_id: String,
    pub type_id: String,
    pub config_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct DspChainPayloadItem {
    plugin_id: String,
    type_id: String,
    config_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
struct DspChainPayloadV1 {
    #[serde(default)]
    pre_mix: Vec<DspChainPayloadItem>,
    #[serde(default)]
    post_mix: Vec<DspChainPayloadItem>,
}

pub fn parse_stage_control_payload(
    schema_id: &str,
    revision: u64,
    payload: &[u8],
) -> Result<Option<Vec<BuiltinDspChainEntry>>, String> {
    if schema_id != DSP_CHAIN_SCHEMA_ID {
        return Ok(None);
    }
    if revision != DSP_CHAIN_SCHEMA_REVISION {
        return Err(format!(
            "unsupported dsp chain revision: {revision}; expected {DSP_CHAIN_SCHEMA_REVISION}"
        ));
    }
    let parsed: DspChainPayloadV1 = serde_json::from_slice(payload)
        .map_err(|e| format!("invalid dsp chain payload json: {e}"))?;
    let mut entries = Vec::with_capacity(parsed.pre_mix.len() + parsed.post_mix.len());
    entries.extend(parsed.pre_mix.into_iter().map(|item| BuiltinDspChainEntry {
        stage: DspChainStage::PreMix,
        plugin_id: item.plugin_id,
        type_id: item.type_id,
        config_json: item.config_json,
    }));
    entries.extend(
        parsed
            .post_mix
            .into_iter()
            .map(|item| BuiltinDspChainEntry {
                stage: DspChainStage::PostMix,
                plugin_id: item.plugin_id,
                type_id: item.type_id,
                config_json: item.config_json,
            }),
    );
    Ok(Some(entries))
}

#[cfg(test)]
mod tests {
    use super::{DspChainStage, parse_stage_control_payload};

    #[test]
    fn parses_v1_payload_into_staged_entries() {
        let payload = br#"{
            "pre_mix":[{"plugin_id":"host","type_id":"eq","config_json":"{}"}],
            "post_mix":[{"plugin_id":"host","type_id":"limiter","config_json":"{}"}]
        }"#;
        let entries = parse_stage_control_payload("stellatune.audio.dsp.chain/v1", 1, payload)
            .expect("parse failed")
            .expect("must match schema");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].stage, DspChainStage::PreMix);
        assert_eq!(entries[1].stage, DspChainStage::PostMix);
    }
}
