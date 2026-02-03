use std::sync::Arc;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputSpec {
    pub sample_rate: u32,
    pub channels: u16,
}

pub trait SampleConsumer: Send + 'static {
    fn pop_sample(&mut self) -> Option<f32>;

    /// Called once per audio callback after the output buffer has been filled.
    ///
    /// `requested` is the number of samples the callback needed, `provided` is the number of
    /// samples actually obtained from the ring buffer.
    ///
    /// This must be lightweight (no allocations/locks/IO).
    fn on_output(&mut self, _requested: usize, _provided: usize) {}
}

#[derive(Debug, Error)]
pub enum OutputError {
    #[error("no default output device")]
    NoDevice,

    #[error("failed to query default output config: {0}")]
    DefaultConfig(#[from] cpal::DefaultStreamConfigError),

    #[error("unsupported stream config: {0}")]
    StreamConfig(#[from] cpal::SupportedStreamConfigsError),

    #[error("failed to build output stream: {0}")]
    BuildStream(#[from] cpal::BuildStreamError),

    #[error("failed to play output stream: {0}")]
    PlayStream(#[from] cpal::PlayStreamError),

    #[error("output device config mismatch: {message}")]
    ConfigMismatch { message: String },
}

pub struct OutputHandle {
    _stream: cpal::Stream,
    spec: OutputSpec,
}

pub fn default_output_spec() -> Result<OutputSpec, OutputError> {
    let host = cpal::default_host();
    let device = host.default_output_device().ok_or(OutputError::NoDevice)?;
    let config = device.default_output_config()?;
    Ok(OutputSpec {
        sample_rate: config.sample_rate(),
        channels: config.channels(),
    })
}

impl OutputHandle {
    pub fn start<C: SampleConsumer, F>(
        mut consumer: C,
        expected_sample_rate: u32,
        on_error: F,
    ) -> Result<Self, OutputError>
    where
        F: Fn(cpal::StreamError) + Send + Sync + 'static,
    {
        let host = cpal::default_host();
        let device = host.default_output_device().ok_or(OutputError::NoDevice)?;
        let config = device.default_output_config()?;

        let sample_rate = config.sample_rate();
        let channels = config.channels();

        if channels != 1 && channels != 2 {
            return Err(OutputError::ConfigMismatch {
                message: format!("output channels = {channels}, only mono/stereo is supported"),
            });
        }

        if sample_rate != expected_sample_rate {
            return Err(OutputError::ConfigMismatch {
                message: format!(
                    "sample rate mismatch: track = {expected_sample_rate}Hz, output = {sample_rate}Hz"
                ),
            });
        }

        let spec = OutputSpec {
            sample_rate,
            channels,
        };

        let stream_config: cpal::StreamConfig = config.clone().into();
        let on_error = Arc::new(on_error);

        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => {
                let on_error = Arc::clone(&on_error);
                device.build_output_stream(
                    &stream_config,
                    move |data: &mut [f32], _| fill_f32(data, &mut consumer),
                    move |err| (on_error)(err),
                    Some(Duration::from_millis(200)),
                )?
            }
            cpal::SampleFormat::I16 => {
                let on_error = Arc::clone(&on_error);
                device.build_output_stream(
                    &stream_config,
                    move |data: &mut [i16], _| fill_i16(data, &mut consumer),
                    move |err| (on_error)(err),
                    Some(Duration::from_millis(200)),
                )?
            }
            cpal::SampleFormat::U16 => {
                let on_error = Arc::clone(&on_error);
                device.build_output_stream(
                    &stream_config,
                    move |data: &mut [u16], _| fill_u16(data, &mut consumer),
                    move |err| (on_error)(err),
                    Some(Duration::from_millis(200)),
                )?
            }
            other => {
                return Err(OutputError::ConfigMismatch {
                    message: format!("unsupported output sample format: {other:?}"),
                });
            }
        };

        stream.play()?;

        Ok(Self {
            _stream: stream,
            spec,
        })
    }

    pub fn spec(&self) -> OutputSpec {
        self.spec
    }
}

fn fill_f32<C: SampleConsumer>(out: &mut [f32], consumer: &mut C) {
    let mut provided = 0usize;
    for slot in out.iter_mut() {
        match consumer.pop_sample() {
            Some(v) => {
                provided += 1;
                *slot = v;
            }
            None => *slot = 0.0,
        }
    }
    consumer.on_output(out.len(), provided);
}

fn fill_i16<C: SampleConsumer>(out: &mut [i16], consumer: &mut C) {
    let mut provided = 0usize;
    for slot in out.iter_mut() {
        match consumer.pop_sample() {
            Some(v) => {
                provided += 1;
                *slot = f32_to_i16(v);
            }
            None => *slot = 0,
        }
    }
    consumer.on_output(out.len(), provided);
}

fn fill_u16<C: SampleConsumer>(out: &mut [u16], consumer: &mut C) {
    let mut provided = 0usize;
    for slot in out.iter_mut() {
        match consumer.pop_sample() {
            Some(v) => {
                provided += 1;
                *slot = f32_to_u16(v);
            }
            None => *slot = 0,
        }
    }
    consumer.on_output(out.len(), provided);
}

fn f32_to_i16(v: f32) -> i16 {
    let v = v.clamp(-1.0, 1.0);
    (v * i16::MAX as f32) as i16
}

fn f32_to_u16(v: f32) -> u16 {
    let v = v.clamp(-1.0, 1.0);
    let normalized = (v + 1.0) * 0.5;
    (normalized * u16::MAX as f32) as u16
}
