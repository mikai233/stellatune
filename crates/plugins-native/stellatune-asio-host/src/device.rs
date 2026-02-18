use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::thread;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait};
use stellatune_asio_proto::{AudioSpec, DeviceCaps, DeviceInfo, SampleFormat};

use crate::state::{DeviceSnapshotEntry, RuntimeState};

pub(crate) const OPEN_RECONFIGURE_SETTLE_MS: u64 = 25;
const LIVE_DEVICE_LOOKUP_ATTEMPTS: usize = 3;
const LIVE_DEVICE_LOOKUP_INTERVAL_MS: u64 = 20;

fn device_id_string(dev: &cpal::Device) -> String {
    dev.id()
        .ok()
        .map(|id| id.to_string())
        .or_else(|| dev.description().ok().map(|d| d.to_string()))
        .unwrap_or_else(|| "unknown".to_string())
}

fn asio_device_id_from_driver_name(name: &str) -> String {
    format!("asio:{name}")
}

fn asio_host() -> Result<cpal::Host, String> {
    #[cfg(all(windows, feature = "asio"))]
    {
        return cpal::host_from_id(cpal::HostId::Asio).map_err(|e| e.to_string());
    }
    #[cfg(not(all(windows, feature = "asio")))]
    {
        Err("ASIO support not built (enable `stellatune-asio-host` feature `asio`)".to_string())
    }
}

fn sort_dedup_device_meta(devices: &mut Vec<(String, String)>) {
    devices.sort_by(|lhs, rhs| {
        lhs.1
            .to_ascii_lowercase()
            .cmp(&rhs.1.to_ascii_lowercase())
            .then_with(|| lhs.0.cmp(&rhs.0))
    });
    devices.dedup_by(|lhs, rhs| device_id_matches(&lhs.0, &rhs.0));
}

fn enumerate_output_device_meta_live() -> Result<Vec<(String, String)>, String> {
    let host = asio_host()?;
    let devs = host.output_devices().map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for dev in devs {
        let id = device_id_string(&dev);
        let name = dev
            .description()
            .ok()
            .map(|d| d.to_string())
            .unwrap_or_else(|| "Unknown ASIO Device".to_string());
        out.push((id, name));
    }
    sort_dedup_device_meta(&mut out);
    Ok(out)
}

#[cfg(all(windows, feature = "asio"))]
fn enumerate_output_device_meta_catalog() -> Result<Vec<(String, String)>, String> {
    let mut out = asio_sys::Asio::new()
        .driver_names()
        .into_iter()
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .map(|name| (asio_device_id_from_driver_name(&name), name))
        .collect::<Vec<_>>();
    sort_dedup_device_meta(&mut out);
    Ok(out)
}

#[cfg(not(all(windows, feature = "asio")))]
fn enumerate_output_device_meta_catalog() -> Result<Vec<(String, String)>, String> {
    enumerate_output_device_meta_live()
}

fn filter_catalog_by_live_cache(
    mut catalog: Vec<(String, String)>,
    live_ids: &[String],
    active_device_id: Option<&str>,
) -> Vec<(String, String)> {
    if live_ids.is_empty() {
        return catalog;
    }
    catalog.retain(|(id, _)| {
        live_ids.iter().any(|live| device_id_matches(id, live))
            || active_device_id
                .map(|active| device_id_matches(id, active))
                .unwrap_or(false)
    });
    catalog
}

fn enumerate_output_device_meta_for_state(
    state: &mut RuntimeState,
) -> Result<Vec<(String, String)>, String> {
    #[cfg(all(windows, feature = "asio"))]
    {
        // When idle, prefer CPAL live enumeration because it excludes stale registry-only
        // ASIO driver names that cannot actually be opened.
        if state.stream.is_none() {
            match enumerate_output_device_meta_live() {
                Ok(live) if !live.is_empty() => {
                    state.last_live_device_ids = live.iter().map(|(id, _)| id.clone()).collect();
                    return Ok(live);
                },
                Ok(_) => {
                    eprintln!(
                        "asio host ListDevices live enumeration returned empty; falling back to catalog"
                    );
                },
                Err(error) => {
                    eprintln!(
                        "asio host ListDevices live enumeration failed; falling back to catalog: {error}"
                    );
                },
            }
        }

        let catalog = enumerate_output_device_meta_catalog()?;
        let filtered = filter_catalog_by_live_cache(
            catalog.clone(),
            &state.last_live_device_ids,
            state.active_device_id.as_deref(),
        );
        if filtered.is_empty() {
            return Ok(catalog);
        }
        if filtered.len() != catalog.len() {
            let removed = catalog.len().saturating_sub(filtered.len());
            eprintln!(
                "asio host ListDevices filtered stale catalog entries: removed={} kept={} active_device={:?}",
                removed,
                filtered.len(),
                state.active_device_id
            );
        }
        return Ok(filtered);
    }

    #[cfg(not(all(windows, feature = "asio")))]
    {
        let _ = state;
        enumerate_output_device_meta_live()
    }
}

fn build_device_caps_for_device(dev: &cpal::Device) -> Result<DeviceCaps, String> {
    let default_cfg = dev.default_output_config().map_err(|e| e.to_string())?;
    let default_spec = AudioSpec {
        sample_rate: default_cfg.sample_rate(),
        channels: default_cfg.channels(),
    };

    let mut rates = Vec::new();
    let mut chans = Vec::new();
    let mut fmts = Vec::new();

    if let Ok(configs) = dev.supported_output_configs() {
        for cfg in configs {
            let min = cfg.min_sample_rate();
            let max = cfg.max_sample_rate();
            // Enumerate common rates within range (small list, but useful for "match track rate").
            for r in [
                8000u32, 11025, 16000, 22050, 32000, 44100, 48000, 88200, 96000, 176400, 192000,
            ] {
                if r >= min && r <= max {
                    rates.push(r);
                }
            }
            rates.push(min);
            rates.push(max);
            rates.push(default_spec.sample_rate);
            chans.push(cfg.channels());
            fmts.push(match cfg.sample_format() {
                cpal::SampleFormat::F32 => SampleFormat::F32,
                cpal::SampleFormat::I16 => SampleFormat::I16,
                cpal::SampleFormat::I32 => SampleFormat::I32,
                cpal::SampleFormat::U16 => SampleFormat::U16,
                _ => continue,
            });
        }
    }

    rates.sort_unstable();
    rates.dedup();
    chans.sort_unstable();
    chans.dedup();
    fmts.sort_unstable_by_key(|f| *f as u8);
    fmts.dedup();

    Ok(DeviceCaps {
        default_spec,
        supported_sample_rates: rates,
        supported_channels: chans,
        supported_formats: fmts,
    })
}

fn compute_selection_session_id(device_id: &str, device_name: &str) -> String {
    let normalized = format!(
        "{}\u{1f}{}",
        normalize_device_id(device_id),
        device_name.trim().to_ascii_lowercase()
    );
    let mut hasher = DefaultHasher::new();
    normalized.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn normalize_device_id(device_id: &str) -> String {
    let trimmed = device_id.trim();
    trimmed
        .strip_prefix("asio:")
        .unwrap_or(trimmed)
        .trim()
        .to_ascii_lowercase()
}

pub(crate) fn device_id_matches(lhs: &str, rhs: &str) -> bool {
    if lhs == rhs {
        return true;
    }
    let lhs_norm = normalize_device_id(lhs);
    let rhs_norm = normalize_device_id(rhs);
    !lhs_norm.is_empty() && lhs_norm == rhs_norm
}

fn find_snapshot_device<'a>(
    state: &'a RuntimeState,
    selection_session_id: &str,
    device_id: &str,
) -> Option<&'a DeviceSnapshotEntry> {
    state.device_snapshot.iter().find(|device| {
        device_id_matches(&device.id, device_id)
            && device.selection_session_id == selection_session_id
    })
}

pub(crate) fn list_devices(state: &mut RuntimeState) -> Result<Vec<DeviceInfo>, String> {
    let devices = enumerate_output_device_meta_for_state(state)?;
    state.device_snapshot = devices
        .into_iter()
        .map(|(id, name)| {
            let selection_session_id = compute_selection_session_id(&id, &name);
            DeviceSnapshotEntry {
                selection_session_id,
                id,
                name,
            }
        })
        .collect();
    Ok(state
        .device_snapshot
        .iter()
        .map(|device| DeviceInfo {
            selection_session_id: device.selection_session_id.clone(),
            id: device.id.clone(),
            name: device.name.clone(),
        })
        .collect())
}

pub(crate) fn validate_selection_session(
    state: &RuntimeState,
    selection_session_id: &str,
    device_id: &str,
) -> Result<(), String> {
    if find_snapshot_device(state, selection_session_id, device_id).is_some() {
        return Ok(());
    }

    if !state.device_snapshot.is_empty() {
        let available = state
            .device_snapshot
            .iter()
            .map(|device| format!("{} ({})", device.id, device.name))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "device not found in current selection snapshot `{selection_session_id}`: {device_id}; snapshot devices: [{available}]"
        ));
    }

    let current_devices = enumerate_output_device_meta_catalog()?;
    let Some((current_id, current_name)) = current_devices
        .iter()
        .find(|(id, _)| device_id_matches(id, device_id))
    else {
        let available = current_devices
            .iter()
            .map(|(id, name)| format!("{id} ({name})"))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "device not found in current selection session `{selection_session_id}`: {device_id}; current devices: [{available}]"
        ));
    };
    let current_session_id = compute_selection_session_id(current_id, current_name);
    if selection_session_id != current_session_id {
        return Err(format!(
            "stale target session: expected `{current_session_id}`, got `{selection_session_id}`. Refresh output sink targets."
        ));
    }
    Ok(())
}

pub(crate) fn find_live_device(device_id: &str) -> Result<cpal::Device, String> {
    let mut last_available = Vec::<String>::new();
    for attempt in 0..LIVE_DEVICE_LOOKUP_ATTEMPTS {
        let host = asio_host()?;
        let devs = host.output_devices().map_err(|e| e.to_string())?;
        let mut available = Vec::<String>::new();
        for dev in devs {
            let id = device_id_string(&dev);
            let name = dev
                .description()
                .ok()
                .map(|d| d.to_string())
                .unwrap_or_else(|| "Unknown ASIO Device".to_string());
            if device_id_matches(&id, device_id) {
                return Ok(dev);
            }
            available.push(format!("{id} ({name})"));
        }
        eprintln!(
            "asio host live lookup miss: attempt={}/{} target={} available=[{}]",
            attempt + 1,
            LIVE_DEVICE_LOOKUP_ATTEMPTS,
            device_id,
            available.join(", ")
        );
        last_available = available;
        if attempt + 1 < LIVE_DEVICE_LOOKUP_ATTEMPTS {
            thread::sleep(Duration::from_millis(LIVE_DEVICE_LOOKUP_INTERVAL_MS));
        }
    }
    Err(format!(
        "device not found after {} attempts: {device_id}; current devices: [{}]",
        LIVE_DEVICE_LOOKUP_ATTEMPTS,
        last_available.join(", ")
    ))
}

pub(crate) fn get_device_caps(
    state: &mut RuntimeState,
    selection_session_id: &str,
    device_id: &str,
) -> Result<DeviceCaps, String> {
    validate_selection_session(state, selection_session_id, device_id)?;
    match find_live_device(device_id) {
        Ok(dev) => build_device_caps_for_device(&dev),
        Err(first_error) => {
            let should_switch = state.stream.is_some()
                && state
                    .active_device_id
                    .as_deref()
                    .map(|active| !device_id_matches(active, device_id))
                    .unwrap_or(true);
            if !should_switch {
                return Err(first_error);
            }

            eprintln!(
                "asio host GetDeviceCaps switching driver context: active_device={:?} target={}",
                state.active_device_id, device_id
            );
            let _ = state.stream.take();
            state.active_device_id = None;
            thread::sleep(Duration::from_millis(OPEN_RECONFIGURE_SETTLE_MS));

            let dev = find_live_device(device_id).map_err(|second_error| {
                format!(
                    "device not found before and after stream release; before={first_error}; after={second_error}"
                )
            })?;
            build_device_caps_for_device(&dev)
        },
    }
}
