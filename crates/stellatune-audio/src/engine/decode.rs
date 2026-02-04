use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender};
use tracing::debug;

use stellatune_decode::{Decoder, TrackSpec};

use crate::engine::config::{
    BUFFER_PREFILL_CAP_MS, RESAMPLE_CHUNK_FRAMES, RESAMPLE_CUTOFF, RESAMPLE_INTERPOLATION,
    RESAMPLE_OVERSAMPLING_FACTOR, RESAMPLE_SINC_LEN, RESAMPLE_WINDOW,
};
use crate::engine::event_hub::EventHub;
use crate::engine::messages::{DecodeCtrl, InternalMsg};
use crate::ring_buffer::RingBufferProducer;

pub(crate) fn decode_thread(
    path: String,
    events: Arc<EventHub>,
    internal_tx: Sender<InternalMsg>,
    ctrl_rx: Receiver<DecodeCtrl>,
    setup_rx: Receiver<DecodeCtrl>,
    spec_tx: Sender<Result<TrackSpec, String>>,
) {
    let t_open = Instant::now();
    let mut decoder = match Decoder::open(&path) {
        Ok(d) => d,
        Err(e) => {
            let _ = spec_tx.send(Err(format!("failed to open decoder: {e}")));
            return;
        }
    };
    debug!("Decoder::open took {}ms", t_open.elapsed().as_millis());

    let spec = decoder.spec();
    let _ = spec_tx.send(Ok(spec));

    let (mut producer, target_sample_rate, target_channels, start_at_ms, output_enabled) = loop {
        crossbeam_channel::select! {
            recv(setup_rx) -> msg => {
                let Ok(ctrl) = msg else { return };
                if let DecodeCtrl::Setup { producer, target_sample_rate, target_channels, start_at_ms, output_enabled } = ctrl {
                    break (producer, target_sample_rate, target_channels, start_at_ms, output_enabled);
                }
            }
            recv(ctrl_rx) -> msg => {
                let Ok(msg) = msg else { return };
                if matches!(msg, DecodeCtrl::Stop) {
                    return;
                }
            }
        }
    };

    let in_channels = spec.channels as usize;
    let out_channels = target_channels as usize;

    let mut base_ms = start_at_ms.max(0);
    if base_ms > 0 {
        let t_skip = Instant::now();
        let frames_to_skip = ((base_ms as i128 * spec.sample_rate as i128) / 1000) as u64;
        if !skip_frames_by_decoding(&mut decoder, frames_to_skip) {
            let _ = internal_tx.send(InternalMsg::Eof);
            return;
        }
        debug!(
            "fast-forward by decoding/discarding: start_at_ms={} took {}ms",
            base_ms,
            t_skip.elapsed().as_millis()
        );
    }

    let t_resampler = Instant::now();
    let mut resampler =
        match create_resampler_if_needed(spec.sample_rate, target_sample_rate, out_channels) {
            Ok(r) => r,
            Err(e) => {
                let _ = internal_tx.send(InternalMsg::Error(e));
                return;
            }
        };
    debug!(
        "resampler init: {} ({}ms)",
        if resampler.is_some() {
            "enabled"
        } else {
            "bypass"
        },
        t_resampler.elapsed().as_millis()
    );

    let mut playing = false;
    let mut frames_written: u64 = 0;
    let mut last_emit = Instant::now();
    let mut decode_pending: Vec<f32> = Vec::new();
    let mut out_pending: Vec<f32> = Vec::new();

    let mut pending_seek: Option<i64> = None;

    'main: loop {
        // During Buffering we gate the output (fill zeros) until the control thread enables it.
        // While gated, avoid overfilling the ring buffer, otherwise we'd decode+resample a large
        // burst upfront which shows up as a CPU spike "when sound starts".
        if playing && !output_enabled.load(Ordering::Acquire) {
            let buffered_frames = (producer.len() / out_channels) as u64;
            let buffered_ms = ((buffered_frames * 1000) / target_sample_rate.max(1) as u64) as i64;
            if buffered_ms >= BUFFER_PREFILL_CAP_MS {
                thread::sleep(Duration::from_millis(5));
                continue;
            }
        }

        if !playing {
            match ctrl_rx.recv() {
                Ok(DecodeCtrl::Play) => {
                    playing = true;
                    last_emit = Instant::now();
                }
                Ok(DecodeCtrl::Pause) => {}
                Ok(DecodeCtrl::SeekMs { position_ms }) => {
                    let target_ms = position_ms.max(0);
                    output_enabled.store(false, Ordering::Release);
                    producer.clear();
                    decode_pending.clear();
                    out_pending.clear();
                    frames_written = 0;
                    base_ms = target_ms;

                    if let Err(e) = decoder.seek_ms(target_ms as u64) {
                        let _ = internal_tx.send(InternalMsg::Error(format!("{e}")));
                        continue;
                    }
                    match create_resampler_if_needed(
                        spec.sample_rate,
                        target_sample_rate,
                        out_channels,
                    ) {
                        Ok(r) => resampler = r,
                        Err(e) => {
                            let _ = internal_tx.send(InternalMsg::Error(e));
                            continue;
                        }
                    }
                }
                Ok(DecodeCtrl::Setup { .. }) => {}
                Ok(DecodeCtrl::Stop) | Err(_) => break,
            }
            continue;
        }

        while let Ok(ctrl) = ctrl_rx.try_recv() {
            match ctrl {
                DecodeCtrl::Pause => {
                    playing = false;
                    break;
                }
                DecodeCtrl::SeekMs { position_ms } => {
                    let target_ms = position_ms.max(0);
                    output_enabled.store(false, Ordering::Release);
                    producer.clear();
                    decode_pending.clear();
                    out_pending.clear();
                    frames_written = 0;
                    base_ms = target_ms;

                    if let Err(e) = decoder.seek_ms(target_ms as u64) {
                        let _ = internal_tx.send(InternalMsg::Error(format!("{e}")));
                        playing = false;
                        break;
                    }
                    match create_resampler_if_needed(
                        spec.sample_rate,
                        target_sample_rate,
                        out_channels,
                    ) {
                        Ok(r) => resampler = r,
                        Err(e) => {
                            let _ = internal_tx.send(InternalMsg::Error(e));
                            playing = false;
                            break;
                        }
                    }
                    last_emit = Instant::now();
                }
                DecodeCtrl::Stop => return,
                DecodeCtrl::Play => {}
                DecodeCtrl::Setup { .. } => {}
            }
        }
        if !playing {
            continue;
        }

        if last_emit.elapsed() >= Duration::from_millis(200) {
            let buffered_frames = (producer.len() / out_channels) as u64;
            let played_frames = frames_written.saturating_sub(buffered_frames);
            let ms = base_ms.saturating_add(
                ((played_frames.saturating_mul(1000)) / target_sample_rate as u64) as i64,
            );
            events.emit(stellatune_core::Event::Position { ms });
            let _ = internal_tx.try_send(InternalMsg::Position(ms));
            last_emit = Instant::now();
        }

        match decoder.next_block(4096) {
            Ok(Some(samples)) => {
                decode_pending.extend_from_slice(&samples);
                if resampler.is_none() {
                    // Channel adaptation only.
                    if in_channels == out_channels {
                        out_pending.extend_from_slice(&decode_pending);
                    } else {
                        out_pending.extend_from_slice(&adapt_channels_interleaved(
                            &decode_pending,
                            in_channels,
                            out_channels,
                        ));
                    }
                    decode_pending.clear();
                    if write_pending(
                        &mut producer,
                        &mut out_pending,
                        &mut frames_written,
                        out_channels,
                        &ctrl_rx,
                        &mut playing,
                        &mut pending_seek,
                    ) {
                        return;
                    }
                    if let Some(seek_ms) = pending_seek.take() {
                        let target_ms = seek_ms.max(0);
                        output_enabled.store(false, Ordering::Release);
                        producer.clear();
                        decode_pending.clear();
                        out_pending.clear();
                        frames_written = 0;
                        base_ms = target_ms;

                        if let Err(e) = decoder.seek_ms(target_ms as u64) {
                            let _ = internal_tx.send(InternalMsg::Error(format!("{e}")));
                            playing = false;
                            continue 'main;
                        }
                        match create_resampler_if_needed(
                            spec.sample_rate,
                            target_sample_rate,
                            out_channels,
                        ) {
                            Ok(r) => resampler = r,
                            Err(e) => {
                                let _ = internal_tx.send(InternalMsg::Error(e));
                                playing = false;
                                continue 'main;
                            }
                        }
                        last_emit = Instant::now();
                        continue 'main;
                    }
                    continue;
                }

                while decode_pending.len() >= RESAMPLE_CHUNK_FRAMES * in_channels {
                    let chunk_in: Vec<f32> = decode_pending
                        .drain(..RESAMPLE_CHUNK_FRAMES * in_channels)
                        .collect();
                    let chunk = if in_channels == out_channels {
                        chunk_in
                    } else {
                        adapt_channels_interleaved(&chunk_in, in_channels, out_channels)
                    };

                    let processed = match resample_interleaved_chunk(
                        resampler.as_mut().expect("checked"),
                        &chunk,
                        out_channels,
                    ) {
                        Ok(v) => v,
                        Err(e) => {
                            let _ = internal_tx.send(InternalMsg::Error(e));
                            return;
                        }
                    };
                    out_pending.extend_from_slice(&processed);

                    if write_pending(
                        &mut producer,
                        &mut out_pending,
                        &mut frames_written,
                        out_channels,
                        &ctrl_rx,
                        &mut playing,
                        &mut pending_seek,
                    ) {
                        return;
                    }
                    if let Some(seek_ms) = pending_seek.take() {
                        let target_ms = seek_ms.max(0);
                        output_enabled.store(false, Ordering::Release);
                        producer.clear();
                        decode_pending.clear();
                        out_pending.clear();
                        frames_written = 0;
                        base_ms = target_ms;

                        if let Err(e) = decoder.seek_ms(target_ms as u64) {
                            let _ = internal_tx.send(InternalMsg::Error(format!("{e}")));
                            playing = false;
                            continue 'main;
                        }
                        match create_resampler_if_needed(
                            spec.sample_rate,
                            target_sample_rate,
                            out_channels,
                        ) {
                            Ok(r) => resampler = r,
                            Err(e) => {
                                let _ = internal_tx.send(InternalMsg::Error(e));
                                playing = false;
                                continue 'main;
                            }
                        }
                        last_emit = Instant::now();
                        continue 'main;
                    }
                    if !playing {
                        break;
                    }
                }
            }
            Ok(None) => {
                if let Some(resampler_inner) = resampler.as_mut() {
                    if !decode_pending.is_empty() {
                        decode_pending.resize(RESAMPLE_CHUNK_FRAMES * in_channels, 0.0);
                        let chunk = if in_channels == out_channels {
                            decode_pending.clone()
                        } else {
                            adapt_channels_interleaved(&decode_pending, in_channels, out_channels)
                        };
                        match resample_interleaved_chunk(resampler_inner, &chunk, out_channels) {
                            Ok(processed) => {
                                out_pending.extend_from_slice(&processed);
                                decode_pending.clear();
                            }
                            Err(e) => {
                                let _ = internal_tx.send(InternalMsg::Error(e));
                                return;
                            }
                        }
                    }
                    while !out_pending.is_empty() {
                        if write_pending(
                            &mut producer,
                            &mut out_pending,
                            &mut frames_written,
                            out_channels,
                            &ctrl_rx,
                            &mut playing,
                            &mut pending_seek,
                        ) {
                            return;
                        }
                        if let Some(seek_ms) = pending_seek.take() {
                            let target_ms = seek_ms.max(0);
                            output_enabled.store(false, Ordering::Release);
                            producer.clear();
                            decode_pending.clear();
                            out_pending.clear();
                            frames_written = 0;
                            base_ms = target_ms;

                            if let Err(e) = decoder.seek_ms(target_ms as u64) {
                                let _ = internal_tx.send(InternalMsg::Error(format!("{e}")));
                                playing = false;
                                continue 'main;
                            }
                            match create_resampler_if_needed(
                                spec.sample_rate,
                                target_sample_rate,
                                out_channels,
                            ) {
                                Ok(r) => resampler = r,
                                Err(e) => {
                                    let _ = internal_tx.send(InternalMsg::Error(e));
                                    playing = false;
                                    continue 'main;
                                }
                            }
                            last_emit = Instant::now();
                            continue 'main;
                        }
                        if !playing {
                            break;
                        }
                    }
                } else if !decode_pending.is_empty() {
                    if in_channels == out_channels {
                        out_pending.extend_from_slice(&decode_pending);
                    } else {
                        out_pending.extend_from_slice(&adapt_channels_interleaved(
                            &decode_pending,
                            in_channels,
                            out_channels,
                        ));
                    }
                    decode_pending.clear();
                    while !out_pending.is_empty() {
                        if write_pending(
                            &mut producer,
                            &mut out_pending,
                            &mut frames_written,
                            out_channels,
                            &ctrl_rx,
                            &mut playing,
                            &mut pending_seek,
                        ) {
                            return;
                        }
                        if let Some(seek_ms) = pending_seek.take() {
                            let target_ms = seek_ms.max(0);
                            output_enabled.store(false, Ordering::Release);
                            producer.clear();
                            decode_pending.clear();
                            out_pending.clear();
                            frames_written = 0;
                            base_ms = target_ms;

                            if let Err(e) = decoder.seek_ms(target_ms as u64) {
                                let _ = internal_tx.send(InternalMsg::Error(format!("{e}")));
                                playing = false;
                                continue 'main;
                            }
                            match create_resampler_if_needed(
                                spec.sample_rate,
                                target_sample_rate,
                                out_channels,
                            ) {
                                Ok(r) => resampler = r,
                                Err(e) => {
                                    let _ = internal_tx.send(InternalMsg::Error(e));
                                    playing = false;
                                    continue 'main;
                                }
                            }
                            last_emit = Instant::now();
                            continue 'main;
                        }
                        if !playing {
                            break;
                        }
                    }
                }
                let _ = internal_tx.send(InternalMsg::Eof);
                break;
            }
            Err(e) => {
                let _ = internal_tx.send(InternalMsg::Error(format!("{e}")));
                break;
            }
        }
    }
}

fn skip_frames_by_decoding(decoder: &mut Decoder, mut frames_to_skip: u64) -> bool {
    // Best-effort: decode and discard samples until reaching the requested frame offset.
    // This is only used during output reinitialization (rare), so it can be slower.
    while frames_to_skip > 0 {
        let want = (frames_to_skip.min(2048)) as usize;
        match decoder.next_block(want) {
            Ok(Some(block)) => {
                let got_frames = (block.len() / decoder.spec().channels as usize) as u64;
                if got_frames == 0 {
                    return false;
                }
                frames_to_skip = frames_to_skip.saturating_sub(got_frames);
            }
            Ok(None) => return false,
            Err(_) => return false,
        }
    }
    true
}

fn write_pending(
    producer: &mut RingBufferProducer<f32>,
    pending: &mut Vec<f32>,
    frames_written: &mut u64,
    channels_per_frame: usize,
    ctrl_rx: &Receiver<DecodeCtrl>,
    playing: &mut bool,
    pending_seek: &mut Option<i64>,
) -> bool {
    let mut offset = 0usize;
    while offset < pending.len() {
        while let Ok(ctrl) = ctrl_rx.try_recv() {
            match ctrl {
                DecodeCtrl::Pause => {
                    *playing = false;
                    break;
                }
                DecodeCtrl::SeekMs { position_ms } => {
                    *pending_seek = Some(position_ms);
                    return false;
                }
                DecodeCtrl::Stop => return true,
                DecodeCtrl::Play => {}
                DecodeCtrl::Setup { .. } => {}
            }
        }
        if !*playing {
            break;
        }

        let written = producer.push_slice(&pending[offset..]);
        offset += written;
        *frames_written = (*frames_written).saturating_add((written / channels_per_frame) as u64);
        if written == 0 {
            thread::sleep(Duration::from_millis(2));
        }
    }

    if offset > 0 {
        pending.drain(..offset);
    }

    false
}

fn create_resampler_if_needed(
    src_rate: u32,
    dst_rate: u32,
    channels: usize,
) -> Result<Option<rubato::Async<f32>>, String> {
    if src_rate == dst_rate {
        return Ok(None);
    }

    use rubato::{Async, FixedAsync, SincInterpolationParameters};

    let params = SincInterpolationParameters {
        sinc_len: RESAMPLE_SINC_LEN,
        f_cutoff: RESAMPLE_CUTOFF,
        oversampling_factor: RESAMPLE_OVERSAMPLING_FACTOR,
        interpolation: RESAMPLE_INTERPOLATION,
        window: RESAMPLE_WINDOW,
    };

    let ratio = dst_rate as f64 / src_rate as f64;
    let resampler = Async::<f32>::new_sinc(
        ratio,
        2.0,
        &params,
        RESAMPLE_CHUNK_FRAMES,
        channels,
        FixedAsync::Input,
    )
    .map_err(|e| format!("failed to create resampler: {e}"))?;
    Ok(Some(resampler))
}

fn resample_interleaved_chunk(
    resampler: &mut rubato::Async<f32>,
    chunk_interleaved: &[f32],
    channels: usize,
) -> Result<Vec<f32>, String> {
    use audioadapter_buffers::direct::InterleavedSlice;
    use rubato::Resampler;

    let frames = chunk_interleaved.len() / channels;
    let input = InterleavedSlice::new(chunk_interleaved, channels, frames)
        .map_err(|e| format!("resample input buffer error: {e}"))?;

    let out = resampler
        .process(&input, 0, None)
        .map_err(|e| format!("resample error: {e}"))?;

    Ok(out.take_data())
}

fn adapt_channels_interleaved(input: &[f32], in_channels: usize, out_channels: usize) -> Vec<f32> {
    if in_channels == out_channels {
        return input.to_vec();
    }

    let frames = input.len() / in_channels;
    match (in_channels, out_channels) {
        (1, 2) => {
            let mut out = Vec::with_capacity(frames * 2);
            for i in 0..frames {
                let s = input[i];
                out.push(s);
                out.push(s);
            }
            out
        }
        (2, 1) => {
            let mut out = Vec::with_capacity(frames);
            for i in 0..frames {
                let l = input[i * 2];
                let r = input[i * 2 + 1];
                out.push((l + r) * 0.5);
            }
            out
        }
        _ => input.to_vec(),
    }
}
