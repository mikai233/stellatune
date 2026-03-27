use std::collections::BTreeMap;

use super::shm::{
    SHM_MIN_CAPACITY, SharedByteRingMapped, parse_shared_memory_endpoint, prepare_shared_memory_env,
};
use super::types::{SidecarTransportKind, SidecarTransportOption, resolve_sidecar_executable};

#[test]
fn parse_shared_memory_endpoint_supports_pair_format() {
    let endpoint = parse_shared_memory_endpoint("tx=C:/tmp/a.shm;rx=C:/tmp/b.shm")
        .expect("endpoint parse should succeed");
    assert_eq!(endpoint.tx_path, "C:/tmp/a.shm");
    assert_eq!(endpoint.rx_path, "C:/tmp/b.shm");
}

#[test]
fn parse_shared_memory_endpoint_supports_single_path() {
    let endpoint =
        parse_shared_memory_endpoint("/tmp/ring.shm").expect("single path should be accepted");
    assert_eq!(endpoint.tx_path, "/tmp/ring.shm");
    assert_eq!(endpoint.rx_path, "/tmp/ring.shm");
}

#[test]
fn shared_byte_ring_write_read_round_trip() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("ring.shm");
    let mut writer =
        SharedByteRingMapped::create(path.as_path(), SHM_MIN_CAPACITY).expect("create ring");
    let mut reader = SharedByteRingMapped::open(path.as_path()).expect("open ring");

    let payload = b"stellatune-sidecar-shm";
    let wrote = writer.write_bytes(payload);
    assert_eq!(wrote, payload.len());

    let mut out = vec![0_u8; payload.len()];
    let read = reader.read_bytes(&mut out);
    assert_eq!(read, payload.len());
    assert_eq!(out, payload);
}

#[test]
fn prepare_shared_memory_env_creates_endpoint_files() {
    let preferred = vec![SidecarTransportOption {
        kind: SidecarTransportKind::SharedMemoryRing,
        priority: 10,
        max_frame_bytes: Some(8192),
    }];
    let mut env = Vec::<(String, String)>::new();
    let mut env_map = BTreeMap::<String, String>::new();
    let mut created_paths = Vec::new();
    #[cfg(unix)]
    let mut created_semaphore_names = Vec::new();
    #[cfg(windows)]
    let mut created_event_handles = Vec::new();

    prepare_shared_memory_env(
        &preferred,
        "STELLATUNE_SIDECAR_DATA_SHARED_MEMORY_RING",
        "STELLATUNE_SIDECAR_DATA_SHM",
        &mut env,
        &mut env_map,
        &mut created_paths,
        #[cfg(unix)]
        &mut created_semaphore_names,
        #[cfg(windows)]
        &mut created_event_handles,
    )
    .expect("prepare env");

    let endpoint = env_map
        .get("STELLATUNE_SIDECAR_DATA_SHARED_MEMORY_RING")
        .expect("full key must exist");
    assert!(endpoint.contains("tx="));
    assert!(endpoint.contains("rx="));
    assert!(endpoint.contains("tx_data_event="));
    assert!(endpoint.contains("tx_space_event="));
    assert!(endpoint.contains("rx_data_event="));
    assert!(endpoint.contains("rx_space_event="));
    assert_eq!(env_map.get("STELLATUNE_SIDECAR_DATA_SHM"), Some(endpoint));

    assert_eq!(created_paths.len(), 2);
    for path in created_paths {
        assert!(path.exists());
        let _ = std::fs::remove_file(path);
    }
    #[cfg(unix)]
    for name in created_semaphore_names {
        let _ = super::shm::unlink_named_semaphore(name.as_str());
    }
}

#[test]
fn resolves_bare_executable_in_plugin_bin_dir() {
    let temp = tempfile::tempdir().expect("create tempdir");
    let root = temp.path();
    let bin_dir = root.join("bin");
    std::fs::create_dir_all(&bin_dir).expect("create bin dir");

    let bare = "stellatune-asio-host";
    let expected = if cfg!(windows) {
        bin_dir.join("stellatune-asio-host.exe")
    } else {
        bin_dir.join("stellatune-asio-host")
    };
    std::fs::write(&expected, b"stub").expect("create sidecar stub");

    let resolved = resolve_sidecar_executable(root, bare).expect("resolve executable");
    assert_eq!(std::path::Path::new(&resolved), expected.as_path());
}

#[test]
fn fails_when_no_candidate_exists() {
    let temp = tempfile::tempdir().expect("create tempdir");
    let err = resolve_sidecar_executable(temp.path(), "stellatune-asio-host")
        .expect_err("missing sidecar should fail");
    assert!(err.to_string().contains("sidecar executable"));
}

#[test]
fn rejects_parent_dir_relative_path() {
    let temp = tempfile::tempdir().expect("create tempdir");
    let err = resolve_sidecar_executable(temp.path(), "../stellatune-asio-host")
        .expect_err("unsafe relative path should fail");
    assert!(err.to_string().contains("unsafe"));
}
