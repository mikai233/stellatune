use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use stellatune_asio_proto::shm::SharedRingMapped;
use stellatune_asio_proto::{AudioSpec, SharedRingFile};
use stellatune_plugin_sdk::{SdkError, SdkResult, resolve_runtime_path};

pub(crate) fn create_ring(
    ring_capacity_ms: u32,
    spec: &AudioSpec,
) -> SdkResult<(SharedRingMapped, SharedRingFile, PathBuf)> {
    let ring_capacity_ms = ring_capacity_ms.max(20);

    let capacity_samples_u64 = (spec.sample_rate as u64)
        .saturating_mul(spec.channels as u64)
        .saturating_mul(ring_capacity_ms as u64)
        / 1000;
    let min_samples = (spec.channels as u64).saturating_mul(512);
    let capacity_samples = capacity_samples_u64.max(min_samples).min(u32::MAX as u64) as u32;

    let base_dir = resolve_runtime_path(".asio")
        .unwrap_or_else(|| std::env::temp_dir().join("stellatune-asio"));
    std::fs::create_dir_all(&base_dir)?;

    let pid = std::process::id();
    let mut created = None;
    for attempt in 0..16u32 {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_micros())
            .unwrap_or(0);
        let path = base_dir.join(format!("ring-{pid}-{ts}-{attempt}.shm"));

        match SharedRingMapped::create(&path, capacity_samples as usize, spec.channels) {
            Ok(map) => {
                created = Some((path, map));
                break;
            },
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(SdkError::Io(format!("failed to create shared ring: {e}"))),
        }
    }

    let Some((path, ring)) = created else {
        return Err(SdkError::msg(
            "failed to create unique ASIO shared ring file",
        ));
    };

    let desc = SharedRingFile {
        path: path.to_string_lossy().to_string(),
        capacity_samples,
    };

    Ok((ring, desc, path))
}
