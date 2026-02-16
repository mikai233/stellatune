use stellatune_plugin_sdk::instance::OutputSinkInstance;
use stellatune_plugin_sdk::update::ConfigUpdatable;
use stellatune_plugin_sdk::{
    OutputSink, SdkError, SdkResult, StAudioSpec, StOutputSinkNegotiatedSpec,
    StOutputSinkRuntimeStatus,
};

use crate::client::{ensure_windows, sidecar_get_device_caps, sidecar_list_devices};
use crate::config::{AsioOutputConfig, AsioOutputTarget, build_negotiated_spec};
use crate::sink::AsioOutputSink;

pub struct AsioOutputSinkInstance {
    pub(crate) config: AsioOutputConfig,
    pub(crate) opened: Option<AsioOutputSink>,
    negotiated_cache: Option<CachedNegotiation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CachedNegotiation {
    target_id: String,
    desired_sample_rate: u32,
    desired_channels: u16,
    negotiated: StOutputSinkNegotiatedSpec,
}

impl AsioOutputSinkInstance {
    fn invalidate_negotiate_cache(&mut self) {
        self.negotiated_cache = None;
    }
}

pub(crate) fn create_instance(config: AsioOutputConfig) -> SdkResult<AsioOutputSinkInstance> {
    ensure_windows()?;
    Ok(AsioOutputSinkInstance {
        config,
        opened: None,
        negotiated_cache: None,
    })
}

impl ConfigUpdatable for AsioOutputSinkInstance {}

impl OutputSinkInstance for AsioOutputSinkInstance {
    fn list_targets_json(&mut self) -> SdkResult<String> {
        ensure_windows()?;
        let devices = sidecar_list_devices(&self.config)?;
        let targets = devices
            .into_iter()
            .map(|d| AsioOutputTarget {
                id: d.id,
                name: Some(d.name),
            })
            .collect::<Vec<_>>();
        stellatune_plugin_sdk::__private::serde_json::to_string(&targets).map_err(SdkError::from)
    }

    fn negotiate_spec_json(
        &mut self,
        target_json: &str,
        desired_spec: StAudioSpec,
    ) -> SdkResult<StOutputSinkNegotiatedSpec> {
        ensure_windows()?;
        let target: AsioOutputTarget =
            stellatune_plugin_sdk::__private::serde_json::from_str(target_json)
                .map_err(SdkError::from)?;
        let desired_sr = desired_spec.sample_rate.max(1);
        let desired_ch = desired_spec.channels.max(1);

        if let Some(cached) = self.negotiated_cache.as_ref()
            && cached.target_id == target.id
            && cached.desired_sample_rate == desired_sr
            && cached.desired_channels == desired_ch
        {
            return Ok(cached.negotiated);
        }

        let caps = sidecar_get_device_caps(&self.config, &target.id)?;
        let negotiated = build_negotiated_spec(desired_spec, &caps, &self.config);
        self.negotiated_cache = Some(CachedNegotiation {
            target_id: target.id,
            desired_sample_rate: desired_sr,
            desired_channels: desired_ch,
            negotiated,
        });
        Ok(negotiated)
    }

    fn open_json(&mut self, target_json: &str, spec: StAudioSpec) -> SdkResult<()> {
        ensure_windows()?;
        let target: AsioOutputTarget =
            stellatune_plugin_sdk::__private::serde_json::from_str(target_json)
                .map_err(SdkError::from)?;
        let sink = AsioOutputSink::open(spec, &self.config, target.id)?;
        self.opened = Some(sink);
        self.invalidate_negotiate_cache();
        Ok(())
    }

    fn write_interleaved_f32(&mut self, channels: u16, samples: &[f32]) -> SdkResult<u32> {
        let sink = self
            .opened
            .as_mut()
            .ok_or_else(|| SdkError::msg("output sink is not open"))?;
        <AsioOutputSink as OutputSink>::write_interleaved_f32(sink, channels, samples)
    }

    fn query_status(&mut self) -> SdkResult<StOutputSinkRuntimeStatus> {
        let sink = self
            .opened
            .as_ref()
            .ok_or_else(|| SdkError::msg("output sink is not open"))?;
        Ok(StOutputSinkRuntimeStatus {
            queued_samples: sink.queued_samples(),
            running: u8::from(sink.started()),
            reserved0: 0,
            reserved1: 0,
        })
    }

    fn flush(&mut self) -> SdkResult<()> {
        let sink = self
            .opened
            .as_mut()
            .ok_or_else(|| SdkError::msg("output sink is not open"))?;
        <AsioOutputSink as OutputSink>::flush(sink)
    }

    fn reset(&mut self) -> SdkResult<()> {
        let sink = self
            .opened
            .as_mut()
            .ok_or_else(|| SdkError::msg("output sink is not open"))?;
        sink.reset_for_disrupt()
    }

    fn close(&mut self) -> SdkResult<()> {
        // Cleanup semantic: closing an output sink instance must deterministically release
        // runtime-owned external resources (ring mapping + sidecar lease via sink drop).
        self.opened = None;
        self.invalidate_negotiate_cache();
        Ok(())
    }
}
