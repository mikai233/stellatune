use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use anyhow::{Context, Result, anyhow};
use tracing::warn;

use crate::frb_generated::StreamSink;
use stellatune_backend_api::runtime::{
    PcmF32Chunk, open_local_transcode_decoder, open_local_transcode_encoder,
};

use super::types::TranscodeProgressEvent;

fn shared_transcode_cancel_flags() -> &'static Mutex<HashMap<String, Arc<AtomicBool>>> {
    static FLAGS: OnceLock<Mutex<HashMap<String, Arc<AtomicBool>>>> = OnceLock::new();
    FLAGS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn register_transcode_cancel_flag(task_id: &str) -> Result<Arc<AtomicBool>> {
    let mut guard = shared_transcode_cancel_flags()
        .lock()
        .map_err(|_| anyhow!("transcode cancel map is poisoned"))?;
    if guard.contains_key(task_id) {
        return Err(anyhow!("transcode task_id `{task_id}` is already running"));
    }
    let flag = Arc::new(AtomicBool::new(false));
    guard.insert(task_id.to_string(), Arc::clone(&flag));
    Ok(flag)
}

fn clear_transcode_cancel_flag(task_id: &str) {
    if let Ok(mut guard) = shared_transcode_cancel_flags().lock() {
        guard.remove(task_id);
    }
}

#[derive(Debug, Clone)]
pub struct TranscodeTrackLocalRequest {
    pub task_id: String,
    pub source_path: String,
    pub output_path: String,
    pub encoder_plugin_id: String,
    pub encoder_type_id: String,
    pub encoder_config_json: String,
    pub encoder_options_json: Option<String>,
}

impl TranscodeTrackLocalRequest {
    pub(crate) fn new(
        task_id: String,
        source_path: String,
        output_path: String,
        encoder_plugin_id: String,
        encoder_type_id: String,
        encoder_config_json: String,
        encoder_options_json: Option<String>,
    ) -> Result<Self> {
        let task_id = task_id.trim().to_string();
        let source_path = source_path.trim().to_string();
        let output_path = output_path.trim().to_string();
        let encoder_plugin_id = encoder_plugin_id.trim().to_string();
        let encoder_type_id = encoder_type_id.trim().to_string();
        if task_id.is_empty() {
            return Err(anyhow!("task_id is empty"));
        }
        if source_path.is_empty() {
            return Err(anyhow!("source_path is empty"));
        }
        if output_path.is_empty() {
            return Err(anyhow!("output_path is empty"));
        }
        if encoder_plugin_id.is_empty() {
            return Err(anyhow!("encoder_plugin_id is empty"));
        }
        if encoder_type_id.is_empty() {
            return Err(anyhow!("encoder_type_id is empty"));
        }
        Ok(Self {
            task_id,
            source_path,
            output_path,
            encoder_plugin_id,
            encoder_type_id,
            encoder_config_json,
            encoder_options_json,
        })
    }
}

struct TranscodeTaskContext {
    request: TranscodeTrackLocalRequest,
    cancel_flag: Arc<AtomicBool>,
    sink: StreamSink<TranscodeProgressEvent>,
}

pub fn transcode_track_local(
    request: TranscodeTrackLocalRequest,
    sink: StreamSink<TranscodeProgressEvent>,
) -> Result<()> {
    let request = TranscodeTrackLocalRequest::new(
        request.task_id,
        request.source_path,
        request.output_path,
        request.encoder_plugin_id,
        request.encoder_type_id,
        request.encoder_config_json,
        request.encoder_options_json,
    )?;
    let cancel_flag = register_transcode_cancel_flag(request.task_id.as_str())?;

    crate::background_runtime::spawn(async move {
        let task_id_for_worker = request.task_id.clone();
        let worker = tokio::task::spawn_blocking(move || {
            run_transcode_track_local_blocking(TranscodeTaskContext {
                request,
                cancel_flag,
                sink,
            })
        });
        match worker.await {
            Ok(()) => {},
            Err(error) => {
                warn!(error = %error, "transcode worker join failed");
            },
        }
        clear_transcode_cancel_flag(task_id_for_worker.as_str());
    });

    Ok(())
}

pub fn transcode_cancel(task_id: String) -> Result<()> {
    let task_id = task_id.trim();
    if task_id.is_empty() {
        return Err(anyhow!("task_id is empty"));
    }
    let guard = shared_transcode_cancel_flags()
        .lock()
        .map_err(|_| anyhow!("transcode cancel map is poisoned"))?;
    if let Some(flag) = guard.get(task_id) {
        flag.store(true, Ordering::Relaxed);
    }
    Ok(())
}

fn run_transcode_track_local_blocking(context: TranscodeTaskContext) {
    const MAX_DECODE_FRAMES: u32 = 8192;
    let TranscodeTaskContext {
        request:
            TranscodeTrackLocalRequest {
                task_id,
                source_path,
                output_path,
                encoder_plugin_id,
                encoder_type_id,
                encoder_config_json,
                encoder_options_json,
            },
        cancel_flag,
        sink,
    } = context;

    let started_at = Instant::now();
    let mut processed_frames = 0u64;
    let mut written_bytes = 0u64;
    let mut total_frames = None::<u64>;
    let mut sample_rate = None::<u32>;
    let mut channels = None::<u16>;

    let source_path_for_failed = source_path.clone();
    let output_path_for_failed = output_path.clone();
    let mut output_file_existed_before = true;

    let result = (|| -> Result<()> {
        ensure_transcode_not_canceled(cancel_flag.as_ref())?;
        let source = Path::new(source_path.as_str());
        if !source.exists() {
            return Err(anyhow!("source file does not exist: {}", source.display()));
        }
        if !source.is_file() {
            return Err(anyhow!("source path is not a file: {}", source.display()));
        }

        let output = Path::new(output_path.as_str());
        if let Some(parent) = output.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create output directory: {}", parent.display())
            })?;
        }
        output_file_existed_before = output.exists();

        let mut decoder = tokio::runtime::Handle::current()
            .block_on(open_local_transcode_decoder(source_path.as_str()))
            .map_err(|error| anyhow!("failed to open local transcode decoder: {error}"))?;
        let decoder_info = decoder.info();

        sample_rate = Some(decoder_info.sample_rate);
        channels = Some(decoder_info.channels);
        total_frames = estimate_total_frames(decoder_info.duration_ms, decoder_info.sample_rate);

        let encoder_config_json = normalize_optional_json(encoder_config_json)?.unwrap_or_default();
        let encoder_options_json = normalize_optional_json_opt(encoder_options_json)?;
        let mut encoder = open_local_transcode_encoder(
            output_path.as_str(),
            encoder_plugin_id.as_str(),
            encoder_type_id.as_str(),
            decoder_info.sample_rate,
            decoder_info.channels,
            decoder_info.metadata.clone(),
            encoder_config_json.as_str(),
            encoder_options_json.as_deref(),
        )
        .map_err(|error| anyhow!("failed to open local transcode encoder: {error}"))?;

        emit_transcode_event(
            &sink,
            TranscodeProgressEvent {
                phase: "started".to_string(),
                message: None,
                source_path: Some(source_path.clone()),
                output_path: Some(output_path.clone()),
                processed_frames,
                total_frames,
                written_bytes,
                sample_rate,
                channels,
                elapsed_ms: Some(started_at.elapsed().as_millis() as u64),
            },
        )?;

        loop {
            ensure_transcode_not_canceled(cancel_flag.as_ref())?;
            let pcm = decoder
                .read_pcm_f32(MAX_DECODE_FRAMES)
                .map_err(|error| anyhow!("decoder read failed: {error}"))?;
            let consumed = encoder
                .write_pcm_f32(PcmF32Chunk {
                    interleaved_f32le: pcm.interleaved_f32le,
                    frames: pcm.frames,
                    eof: pcm.eof,
                })
                .map_err(|error| anyhow!("encoder write failed: {error}"))?;
            if consumed != pcm.frames {
                return Err(anyhow!(
                    "encoder partial consume is not supported yet (consumed={} frames={})",
                    consumed,
                    pcm.frames
                ));
            }
            processed_frames = processed_frames.saturating_add(consumed as u64);
            written_bytes = encoder.written_bytes();
            emit_transcode_event(
                &sink,
                TranscodeProgressEvent {
                    phase: "progress".to_string(),
                    message: None,
                    source_path: Some(source_path.clone()),
                    output_path: Some(output_path.clone()),
                    processed_frames,
                    total_frames,
                    written_bytes,
                    sample_rate,
                    channels,
                    elapsed_ms: Some(started_at.elapsed().as_millis() as u64),
                },
            )?;

            if pcm.eof {
                break;
            }
        }

        encoder
            .finish()
            .map_err(|error| anyhow!("encoder finish failed: {error}"))?;
        written_bytes = encoder.written_bytes();

        let _ = sink.add(TranscodeProgressEvent {
            phase: "completed".to_string(),
            message: None,
            source_path: Some(source_path.clone()),
            output_path: Some(output_path.clone()),
            processed_frames,
            total_frames,
            written_bytes,
            sample_rate,
            channels,
            elapsed_ms: Some(started_at.elapsed().as_millis() as u64),
        });

        // Keep best-effort explicit close; handles also clean up on drop.
        let _ = encoder.close();
        let _ = decoder.close();
        Ok(())
    })();

    if let Err(error) = result {
        let canceled = cancel_flag.load(Ordering::Relaxed);
        warn!(
            task_id = %task_id,
            source_path = %source_path_for_failed,
            output_path = %output_path_for_failed,
            error = %error,
            "transcode_track_local failed"
        );
        if !output_file_existed_before {
            let _ = fs::remove_file(output_path_for_failed.as_str());
        }
        let _ = sink.add(TranscodeProgressEvent {
            phase: if canceled { "canceled" } else { "failed" }.to_string(),
            message: if canceled {
                None
            } else {
                Some(error.to_string())
            },
            source_path: Some(source_path_for_failed),
            output_path: Some(output_path_for_failed),
            processed_frames,
            total_frames,
            written_bytes,
            sample_rate,
            channels,
            elapsed_ms: Some(started_at.elapsed().as_millis() as u64),
        });
    }
}

fn normalize_optional_json(raw: String) -> Result<Option<String>> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let value = serde_json::from_str::<serde_json::Value>(trimmed)
        .map_err(|e| anyhow!("invalid json payload: {e}"))?;
    let normalized = serde_json::to_string(&value).map_err(|e| anyhow!("serialize json: {e}"))?;
    Ok(Some(normalized))
}

fn normalize_optional_json_opt(raw: Option<String>) -> Result<Option<String>> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    normalize_optional_json(raw)
}

fn estimate_total_frames(duration_ms: Option<u64>, sample_rate: u32) -> Option<u64> {
    let duration_ms = duration_ms?;
    let sample_rate = sample_rate.max(1);
    let frames = (duration_ms as u128)
        .saturating_mul(sample_rate as u128)
        .saturating_div(1000);
    Some(frames.min(u64::MAX as u128) as u64)
}

fn emit_transcode_event(
    sink: &StreamSink<TranscodeProgressEvent>,
    event: TranscodeProgressEvent,
) -> Result<()> {
    sink.add(event)
        .map_err(|_| anyhow!("transcode progress stream closed"))
}

fn ensure_transcode_not_canceled(cancel_flag: &AtomicBool) -> Result<()> {
    if cancel_flag.load(Ordering::Relaxed) {
        return Err(anyhow!("transcode canceled"));
    }
    Ok(())
}
