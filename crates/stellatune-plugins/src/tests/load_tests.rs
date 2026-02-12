use super::{cleanup_stale_shadow_libraries_in_dir, parse_shadow_file_key, sanitize_plugin_id};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn unique_temp_dir(suffix: &str) -> PathBuf {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!(
        "stellatune-shadow-cleanup-test-{}-{ts}-{suffix}",
        std::process::id()
    ))
}

fn build_shadow_file_name(pid: u32, seq: u64, base: &str) -> String {
    format!("{}-{pid}-{seq}-{base}", 1_700_000_000_000_u64)
}

fn touch(path: &Path) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create parent dir");
    }
    std::fs::write(path, b"x").expect("write temp file");
}

#[test]
fn sanitize_plugin_id_replaces_invalid_chars() {
    assert_eq!(sanitize_plugin_id(" dev/plug in "), "dev_plug_in");
    assert_eq!(sanitize_plugin_id(""), "unknown-plugin");
}

#[test]
fn parse_shadow_file_key_requires_expected_prefix() {
    assert!(parse_shadow_file_key("1700-123-1-a.dll").is_some());
    assert!(parse_shadow_file_key("a-123-1-a.dll").is_none());
    assert!(parse_shadow_file_key("1700-123-a-a.dll").is_none());
    assert!(parse_shadow_file_key("1700-123-1-").is_none());
}

#[test]
fn cleanup_deletes_stale_and_keeps_active() {
    let root = unique_temp_dir("deletes-stale");
    let plugin_dir = root.join("dev.test.plugin");
    let active_path = plugin_dir.join(build_shadow_file_name(std::process::id(), 1, "a.dll"));
    let stale_current = plugin_dir.join(build_shadow_file_name(std::process::id(), 2, "b.dll"));
    let stale_other = plugin_dir.join(build_shadow_file_name(999_999, 3, "c.dll"));
    let unknown_name = plugin_dir.join("not-a-shadow-name.dll");
    touch(&active_path);
    touch(&stale_current);
    touch(&stale_other);
    touch(&unknown_name);

    let protected = HashSet::from([active_path.clone()]);
    let report = cleanup_stale_shadow_libraries_in_dir(&root, &protected, Duration::ZERO, 1000);

    assert_eq!(report.deleted, 2);
    assert_eq!(report.skipped_active, 1);
    assert_eq!(report.skipped_unrecognized, 1);
    assert!(active_path.exists());
    assert!(!stale_current.exists());
    assert!(!stale_other.exists());
    assert!(unknown_name.exists());

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn cleanup_respects_grace_period_for_current_process() {
    let root = unique_temp_dir("grace");
    let plugin_dir = root.join("dev.test.plugin");
    let current = plugin_dir.join(build_shadow_file_name(std::process::id(), 7, "x.dll"));
    touch(&current);

    let protected = HashSet::<PathBuf>::new();
    let report =
        cleanup_stale_shadow_libraries_in_dir(&root, &protected, Duration::from_secs(3600), 1000);

    assert_eq!(report.deleted, 0);
    assert_eq!(report.skipped_recent_current_process, 1);
    assert!(current.exists());

    let _ = std::fs::remove_dir_all(root);
}

#[cfg(windows)]
#[test]
fn cleanup_failure_can_retry_after_handle_released() {
    use std::fs::OpenOptions;
    use std::os::windows::fs::OpenOptionsExt;

    const FILE_SHARE_READ: u32 = 0x0000_0001;
    const FILE_SHARE_WRITE: u32 = 0x0000_0002;

    let root = unique_temp_dir("retry-after-failure");
    let plugin_dir = root.join("dev.test.plugin");
    let stale = plugin_dir.join(build_shadow_file_name(std::process::id(), 11, "locked.dll"));
    touch(&stale);

    // Intentionally deny FILE_SHARE_DELETE so first cleanup attempt fails on Windows.
    let locked = OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .open(&stale)
        .expect("open lock file");

    let protected = HashSet::<PathBuf>::new();
    let first = cleanup_stale_shadow_libraries_in_dir(&root, &protected, Duration::ZERO, 1000);
    assert_eq!(first.deleted, 0);
    assert!(first.failed >= 1);
    assert!(stale.exists());

    drop(locked);
    let second = cleanup_stale_shadow_libraries_in_dir(&root, &protected, Duration::ZERO, 1000);
    assert!(second.deleted >= 1);
    assert!(!stale.exists());

    let _ = std::fs::remove_dir_all(root);
}
