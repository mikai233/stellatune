use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

use serde_json::Value;
use stellatune_plugin_sdk::instance::{DecoderDescriptor, DecoderInstance};
use stellatune_plugin_sdk::update::{ConfigUpdatable, UpdatePlan};
use stellatune_plugin_sdk::{
    ST_DECODER_INFO_FLAG_HAS_DURATION, ST_DECODER_INFO_FLAG_SEEKABLE, SdkError, SdkResult,
    StAudioSpec, StDecoderInfo,
};

pub struct BetaDecoder {
    build: &'static str,
    gain: Arc<AtomicI32>,
    running: Arc<AtomicBool>,
    beats: Arc<AtomicU64>,
    worker: Option<thread::JoinHandle<()>>,
}

impl BetaDecoder {
    fn from_gain(build: &'static str, gain: i32) -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let beats = Arc::new(AtomicU64::new(0));
        let thread_running = Arc::clone(&running);
        let thread_beats = Arc::clone(&beats);
        let worker = thread::Builder::new()
            .name(format!("beta-bg-{build}"))
            .spawn(move || {
                while thread_running.load(Ordering::Relaxed) {
                    thread_beats.fetch_add(1, Ordering::Relaxed);
                    thread::sleep(Duration::from_millis(5));
                }
            })
            .ok();

        Self {
            build,
            gain: Arc::new(AtomicI32::new(gain)),
            running,
            beats,
            worker,
        }
    }

    fn gain_from_json(new_config_json: &str) -> SdkResult<i32> {
        let parsed: Value =
            serde_json::from_str(new_config_json).map_err(|e| SdkError::msg(e.to_string()))?;
        let gain = parsed
            .get("gain")
            .and_then(Value::as_i64)
            .ok_or_else(|| SdkError::msg("missing integer `gain`"))?;
        Ok(gain as i32)
    }
}

impl Drop for BetaDecoder {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.worker.take() {
            let _ = handle.join();
        }
    }
}

impl ConfigUpdatable for BetaDecoder {
    fn plan_config_update_json(&self, new_config_json: &str) -> SdkResult<UpdatePlan> {
        let parsed: Value =
            serde_json::from_str(new_config_json).map_err(|e| SdkError::msg(e.to_string()))?;
        if parsed.get("gain").and_then(Value::as_i64).is_some() {
            return Ok(UpdatePlan::hot_apply());
        }
        Ok(UpdatePlan::reject("missing integer `gain`"))
    }

    fn apply_config_update_json(&mut self, new_config_json: &str) -> SdkResult<()> {
        let gain = Self::gain_from_json(new_config_json)?;
        self.gain.store(gain, Ordering::Relaxed);
        Ok(())
    }
}

impl DecoderInstance for BetaDecoder {
    fn get_info(&self) -> StDecoderInfo {
        StDecoderInfo {
            spec: StAudioSpec {
                sample_rate: 44_100,
                channels: 2,
                reserved: 0,
            },
            duration_ms: 0,
            encoder_delay_frames: 0,
            encoder_padding_frames: 0,
            flags: ST_DECODER_INFO_FLAG_SEEKABLE | ST_DECODER_INFO_FLAG_HAS_DURATION,
            reserved: 0,
        }
    }

    fn get_metadata_json(&self) -> SdkResult<Option<String>> {
        Ok(Some(
            serde_json::json!({
                "build": self.build,
                "gain": self.gain.load(Ordering::Relaxed),
                "beats": self.beats.load(Ordering::Relaxed),
            })
            .to_string(),
        ))
    }

    fn read_interleaved_f32(
        &mut self,
        frames: u32,
        out_interleaved: &mut [f32],
    ) -> SdkResult<(u32, bool)> {
        let channels = 2usize;
        let write = frames as usize * channels;
        if out_interleaved.len() < write {
            return Err(SdkError::msg("output buffer too small"));
        }
        let v = self.gain.load(Ordering::Relaxed) as f32;
        for sample in &mut out_interleaved[..write] {
            *sample = v;
        }
        Ok((frames, false))
    }
}

impl DecoderDescriptor for BetaDecoder {
    type Config = Value;
    type Instance = BetaDecoder;

    const TYPE_ID: &'static str = "hot";
    const DISPLAY_NAME: &'static str = "Beta Hot Decoder";
    const CONFIG_SCHEMA_JSON: &'static str =
        r#"{"type":"object","properties":{"gain":{"type":"integer"}}}"#;
    const DEFAULT_CONFIG_JSON: &'static str = r#"{"gain":10}"#;

    fn default_config() -> Self::Config {
        serde_json::json!({ "gain": 10 })
    }

    fn create(config: Self::Config) -> SdkResult<Self::Instance> {
        let gain = config.get("gain").and_then(Value::as_i64).unwrap_or(10) as i32;
        Ok(BetaDecoder::from_gain("beta-v1", gain))
    }
}

stellatune_plugin_sdk::export_plugin! {
    id: "dev.stellatune.test.multi.beta",
    name: "Multi Beta V1",
    version: (0, 1, 0),
    decoders: [
        hot => BetaDecoder,
    ],
    dsps: [],
    source_catalogs: [],
    lyrics_providers: [],
    output_sinks: [],
}
