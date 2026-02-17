use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossbeam_channel::{Receiver, Sender};
use serde_json::Value;
use stellatune_plugins::install_plugin_from_artifact;
use stellatune_plugins::runtime::handle::PluginRuntimeHandle;
use stellatune_plugins::runtime::worker_controller::WorkerConfigUpdateOutcome;
use stellatune_plugins::runtime::worker_endpoint::DecoderWorkerEndpoint;

struct FixtureArtifacts {
    alpha_v1: PathBuf,
    alpha_next: PathBuf,
    beta_v1: PathBuf,
}

#[derive(Debug, Clone, Default)]
struct WorkerSnapshot {
    has_instance: bool,
    build: Option<String>,
    gain: Option<i64>,
    beats: Option<u64>,
}

enum WorkerRequest {
    Snapshot(Sender<WorkerSnapshot>),
    HotApply {
        config_json: String,
        reply: Sender<Result<WorkerConfigUpdateOutcome>>,
    },
    Stop(Sender<()>),
}

struct DecoderWorkerHandle {
    tx: Sender<WorkerRequest>,
    join: thread::JoinHandle<()>,
}

impl DecoderWorkerHandle {
    fn snapshot(&self) -> WorkerSnapshot {
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.tx
            .send(WorkerRequest::Snapshot(tx))
            .expect("send snapshot request");
        rx.recv().expect("recv snapshot")
    }

    fn hot_apply(&self, config_json: &str) -> Result<WorkerConfigUpdateOutcome> {
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.tx
            .send(WorkerRequest::HotApply {
                config_json: config_json.to_string(),
                reply: tx,
            })
            .expect("send hot apply request");
        rx.recv().expect("recv hot apply reply")
    }

    fn stop(self) {
        let (tx, rx) = crossbeam_channel::bounded(1);
        let _ = self.tx.send(WorkerRequest::Stop(tx));
        let _ = rx.recv_timeout(Duration::from_secs(1));
        let _ = self.join.join();
    }
}

static TEST_MUTEX: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
static FIXTURES: OnceLock<FixtureArtifacts> = OnceLock::new();

#[tokio::test(flavor = "multi_thread")]
async fn multi_plugin_workers_reload_disable_hot_apply_external_flow() {
    let _guard = TEST_MUTEX
        .get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await;

    let runtime = PluginRuntimeHandle::new_with_default_host();

    let temp = tempfile::tempdir().expect("create temp dir");
    let plugins_dir = temp.path().join("plugins");
    std::fs::create_dir_all(&plugins_dir).expect("create plugins dir");

    let fixtures = fixture_artifacts();
    let alpha_installed = install_plugin_from_artifact(&plugins_dir, &fixtures.alpha_v1)
        .expect("install alpha fixture v1");
    let beta_installed = install_plugin_from_artifact(&plugins_dir, &fixtures.beta_v1)
        .expect("install beta fixture v1");

    runtime
        .reload_dir_from_state(&plugins_dir)
        .await
        .expect("initial reload from state");

    let alpha_endpoint = runtime
        .bind_decoder_worker_endpoint(&alpha_installed.id, "hot")
        .await
        .expect("bind alpha worker endpoint");
    let beta_endpoint = runtime
        .bind_decoder_worker_endpoint(&beta_installed.id, "hot")
        .await
        .expect("bind beta worker endpoint");

    let alpha_worker = spawn_decoder_worker(alpha_endpoint, r#"{"gain":1}"#);
    let beta_worker = spawn_decoder_worker(beta_endpoint, r#"{"gain":10}"#);

    let alpha_ready = wait_for_snapshot(
        &alpha_worker,
        Duration::from_secs(4),
        |s| s.has_instance && s.build.as_deref() == Some("alpha-v1") && s.beats.unwrap_or(0) > 2,
        "alpha worker should start with v1",
    );
    assert_eq!(alpha_ready.gain, Some(1));

    let beta_ready = wait_for_snapshot(
        &beta_worker,
        Duration::from_secs(4),
        |s| s.has_instance && s.build.as_deref() == Some("beta-v1") && s.beats.unwrap_or(0) > 2,
        "beta worker should start with v1",
    );
    assert_eq!(beta_ready.gain, Some(10));

    let hot_outcome = alpha_worker
        .hot_apply(r#"{"gain":7}"#)
        .expect("alpha hot apply should succeed");
    assert!(matches!(
        hot_outcome,
        WorkerConfigUpdateOutcome::Applied { .. }
    ));

    let alpha_hot = wait_for_snapshot(
        &alpha_worker,
        Duration::from_secs(3),
        |s| s.gain == Some(7),
        "alpha gain should become 7 after hot apply",
    );
    assert_eq!(alpha_hot.build.as_deref(), Some("alpha-v1"));

    thread::sleep(Duration::from_millis(50));
    std::fs::copy(&fixtures.alpha_next, &alpha_installed.library_path)
        .expect("overwrite alpha library with v2");

    runtime
        .reload_dir_from_state(&plugins_dir)
        .await
        .expect("reload after alpha dll changed");

    let alpha_reloaded = wait_for_snapshot(
        &alpha_worker,
        Duration::from_secs(4),
        |s| s.has_instance && s.build.as_deref() == Some("alpha-v2") && s.gain == Some(7),
        "alpha worker should recreate into v2 and keep desired config",
    );
    assert!(alpha_reloaded.beats.unwrap_or(0) > 0);

    runtime.set_plugin_enabled(&beta_installed.id, false).await;

    let beta_still_running = wait_for_snapshot(
        &beta_worker,
        Duration::from_secs(4),
        |s| s.has_instance && s.gain == Some(20),
        "beta worker should keep existing instance after plugin disabled",
    );
    assert!(beta_still_running.has_instance);

    let alpha_still_running = alpha_worker.snapshot();
    assert!(alpha_still_running.has_instance);
    assert_eq!(alpha_still_running.build.as_deref(), Some("alpha-v2"));

    let beta_factory_only = runtime
        .bind_decoder_worker_endpoint(&beta_installed.id, "hot")
        .await
        .expect("bind beta endpoint for direct-create check");
    let beta_create_err = match beta_factory_only.factory.create_instance(r#"{"gain":11}"#) {
        Ok(_) => panic!("disabled beta should reject create_instance"),
        Err(err) => err,
    };
    assert!(beta_create_err.to_string().contains("has no active lease"));

    alpha_worker.stop();
    beta_worker.stop();

    runtime.set_plugin_enabled(&beta_installed.id, true).await;
    let _ = runtime.shutdown_and_cleanup().await;
    runtime.cleanup_shadow_copies_now().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn multi_plugin_disable_and_hot_apply_are_isolated_between_workers() {
    let _guard = TEST_MUTEX
        .get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await;

    let runtime = PluginRuntimeHandle::new_with_default_host();

    let temp = tempfile::tempdir().expect("create temp dir");
    let plugins_dir = temp.path().join("plugins");
    std::fs::create_dir_all(&plugins_dir).expect("create plugins dir");

    let fixtures = fixture_artifacts();
    let alpha_installed = install_plugin_from_artifact(&plugins_dir, &fixtures.alpha_v1)
        .expect("install alpha fixture v1");
    let beta_installed = install_plugin_from_artifact(&plugins_dir, &fixtures.beta_v1)
        .expect("install beta fixture v1");

    runtime
        .reload_dir_from_state(&plugins_dir)
        .await
        .expect("initial reload from state");

    let alpha_endpoint = runtime
        .bind_decoder_worker_endpoint(&alpha_installed.id, "hot")
        .await
        .expect("bind alpha worker endpoint");
    let beta_endpoint = runtime
        .bind_decoder_worker_endpoint(&beta_installed.id, "hot")
        .await
        .expect("bind beta worker endpoint");

    let alpha_worker = spawn_decoder_worker(alpha_endpoint, r#"{"gain":2}"#);
    let beta_worker = spawn_decoder_worker(beta_endpoint, r#"{"gain":20}"#);

    let _ = wait_for_snapshot(
        &alpha_worker,
        Duration::from_secs(4),
        |s| s.has_instance && s.gain == Some(2),
        "alpha should start",
    );
    let _ = wait_for_snapshot(
        &beta_worker,
        Duration::from_secs(4),
        |s| s.has_instance && s.gain == Some(20),
        "beta should start",
    );

    let alpha_hot = alpha_worker
        .hot_apply(r#"{"gain":3}"#)
        .expect("alpha hot apply should succeed");
    let beta_hot = beta_worker
        .hot_apply(r#"{"gain":21}"#)
        .expect("beta hot apply should succeed");
    assert!(matches!(
        alpha_hot,
        WorkerConfigUpdateOutcome::Applied { .. }
    ));
    assert!(matches!(
        beta_hot,
        WorkerConfigUpdateOutcome::Applied { .. }
    ));

    runtime.set_plugin_enabled(&alpha_installed.id, false).await;

    let alpha_still_running = wait_for_snapshot(
        &alpha_worker,
        Duration::from_secs(4),
        |s| s.has_instance && s.gain == Some(3),
        "alpha should keep existing instance after disable",
    );
    assert!(alpha_still_running.has_instance);

    let beta_still = wait_for_snapshot(
        &beta_worker,
        Duration::from_secs(4),
        |s| s.has_instance && s.gain == Some(21),
        "beta should remain running with last gain",
    );
    assert_eq!(beta_still.build.as_deref(), Some("beta-v1"));

    let beta_hot_after_alpha_disable = beta_worker
        .hot_apply(r#"{"gain":33}"#)
        .expect("beta hot apply after alpha disable should succeed");
    assert!(matches!(
        beta_hot_after_alpha_disable,
        WorkerConfigUpdateOutcome::Applied { .. }
    ));

    let beta_updated = wait_for_snapshot(
        &beta_worker,
        Duration::from_secs(4),
        |s| s.has_instance && s.gain == Some(33),
        "beta should accept further hot apply",
    );
    assert_eq!(beta_updated.build.as_deref(), Some("beta-v1"));

    alpha_worker.stop();
    beta_worker.stop();

    runtime.set_plugin_enabled(&alpha_installed.id, true).await;
    let _ = runtime.shutdown_and_cleanup().await;
    runtime.cleanup_shadow_copies_now().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn multi_plugin_reload_disable_hot_apply_stress_rounds() {
    let _guard = TEST_MUTEX
        .get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await;

    let runtime = PluginRuntimeHandle::new_with_default_host();

    let temp = tempfile::tempdir().expect("create temp dir");
    let plugins_dir = temp.path().join("plugins");
    std::fs::create_dir_all(&plugins_dir).expect("create plugins dir");

    let fixtures = fixture_artifacts();
    let alpha_installed = install_plugin_from_artifact(&plugins_dir, &fixtures.alpha_v1)
        .expect("install alpha fixture v1");
    let beta_installed = install_plugin_from_artifact(&plugins_dir, &fixtures.beta_v1)
        .expect("install beta fixture v1");

    runtime
        .reload_dir_from_state(&plugins_dir)
        .await
        .expect("initial reload from state");

    let alpha_endpoint = runtime
        .bind_decoder_worker_endpoint(&alpha_installed.id, "hot")
        .await
        .expect("bind alpha worker endpoint");
    let beta_endpoint = runtime
        .bind_decoder_worker_endpoint(&beta_installed.id, "hot")
        .await
        .expect("bind beta worker endpoint");

    let alpha_worker = spawn_decoder_worker(alpha_endpoint, r#"{"gain":1}"#);
    let beta_worker = spawn_decoder_worker(beta_endpoint, r#"{"gain":10}"#);

    let _ = wait_for_snapshot(
        &alpha_worker,
        Duration::from_secs(4),
        |s| s.has_instance && s.build.as_deref() == Some("alpha-v1"),
        "alpha should start at v1",
    );
    let _ = wait_for_snapshot(
        &beta_worker,
        Duration::from_secs(4),
        |s| s.has_instance && s.build.as_deref() == Some("beta-v1"),
        "beta should start at v1",
    );

    const ROUNDS: usize = 12;
    for round in 0..ROUNDS {
        let alpha_gain = 100 + round as i64;
        let beta_gain = 200 + round as i64;

        let alpha_outcome = alpha_worker
            .hot_apply(&format!(r#"{{"gain":{alpha_gain}}}"#))
            .expect("alpha hot apply should succeed");
        let beta_outcome = beta_worker
            .hot_apply(&format!(r#"{{"gain":{beta_gain}}}"#))
            .expect("beta hot apply should succeed");
        assert!(matches!(
            alpha_outcome,
            WorkerConfigUpdateOutcome::Applied { .. }
        ));
        assert!(matches!(
            beta_outcome,
            WorkerConfigUpdateOutcome::Applied { .. }
        ));

        let _ = wait_for_snapshot(
            &alpha_worker,
            Duration::from_secs(3),
            |s| s.has_instance && s.gain == Some(alpha_gain),
            "alpha gain should update after hot apply",
        );
        let _ = wait_for_snapshot(
            &beta_worker,
            Duration::from_secs(3),
            |s| s.has_instance && s.gain == Some(beta_gain),
            "beta gain should update after hot apply",
        );

        let (alpha_lib_src, expected_build) = if round % 2 == 0 {
            (&fixtures.alpha_next, "alpha-v2")
        } else {
            (&fixtures.alpha_v1, "alpha-v1")
        };
        std::thread::sleep(Duration::from_millis(20));
        std::fs::copy(alpha_lib_src, &alpha_installed.library_path)
            .expect("swap alpha plugin library");

        runtime
            .reload_dir_from_state(&plugins_dir)
            .await
            .expect("reload after alpha dll swap");

        let _ = wait_for_snapshot(
            &alpha_worker,
            Duration::from_secs(4),
            |s| {
                s.has_instance
                    && s.build.as_deref() == Some(expected_build)
                    && s.gain == Some(alpha_gain)
            },
            "alpha should recreate with swapped build and keep desired gain",
        );

        runtime.set_plugin_enabled(&beta_installed.id, false).await;
        let _ = wait_for_snapshot(
            &beta_worker,
            Duration::from_secs(4),
            |s| s.has_instance && s.gain == Some(beta_gain),
            "beta should keep existing instance on disable",
        );

        runtime.set_plugin_enabled(&beta_installed.id, true).await;
        let beta_reactivate = beta_worker
            .hot_apply(&format!(r#"{{"gain":{beta_gain}}}"#))
            .expect("beta reactivation hot apply should return");
        assert!(matches!(
            beta_reactivate,
            WorkerConfigUpdateOutcome::Applied { .. }
                | WorkerConfigUpdateOutcome::DeferredNoInstance
        ));
        let _ = wait_for_snapshot(
            &beta_worker,
            Duration::from_secs(4),
            |s| {
                s.has_instance && s.build.as_deref() == Some("beta-v1") && s.gain == Some(beta_gain)
            },
            "beta should be recreated after re-enable + demand",
        );

        let _ = runtime.collect_retired_module_leases_by_refcount().await;
    }

    alpha_worker.stop();
    beta_worker.stop();
    let _ = runtime.shutdown_and_cleanup().await;
    runtime.cleanup_shadow_copies_now().await;
}

fn spawn_decoder_worker(
    endpoint: DecoderWorkerEndpoint,
    initial_config_json: &str,
) -> DecoderWorkerHandle {
    let (tx, rx): (Sender<WorkerRequest>, Receiver<WorkerRequest>) = crossbeam_channel::unbounded();
    let init = initial_config_json.to_string();

    let join = thread::Builder::new()
        .name("multi-plugin-test-worker".to_string())
        .spawn(move || {
            let (mut controller, control_rx) = endpoint.into_controller(init);
            let mut stop = false;

            while !stop {
                while let Ok(control) = control_rx.try_recv() {
                    controller.on_control_message(control);
                }

                while let Ok(req) = rx.try_recv() {
                    match req {
                        WorkerRequest::Snapshot(reply) => {
                            let _ = reply.send(snapshot_from_controller(&mut controller));
                        },
                        WorkerRequest::HotApply { config_json, reply } => {
                            let out = controller.apply_config_update(config_json);
                            let _ = reply.send(out);
                        },
                        WorkerRequest::Stop(reply) => {
                            stop = true;
                            let _ = reply.send(());
                        },
                    }
                }

                let _ = controller.apply_pending();
                if let Some(instance) = controller.instance_mut() {
                    let _ = instance.read_interleaved_f32(16);
                }

                thread::sleep(Duration::from_millis(10));
            }
        })
        .expect("spawn worker thread");

    DecoderWorkerHandle { tx, join }
}

fn snapshot_from_controller(
    controller: &mut stellatune_plugins::runtime::worker_endpoint::DecoderWorkerController,
) -> WorkerSnapshot {
    let Some(instance) = controller.instance() else {
        return WorkerSnapshot::default();
    };

    let raw = instance
        .get_metadata_json()
        .ok()
        .flatten()
        .unwrap_or_else(|| "{}".to_string());
    let parsed: Value = serde_json::from_str(&raw).unwrap_or_else(|_| serde_json::json!({}));
    WorkerSnapshot {
        has_instance: true,
        build: parsed
            .get("build")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        gain: parsed.get("gain").and_then(Value::as_i64),
        beats: parsed.get("beats").and_then(Value::as_u64),
    }
}

fn wait_for_snapshot(
    worker: &DecoderWorkerHandle,
    timeout: Duration,
    predicate: impl Fn(&WorkerSnapshot) -> bool,
    message: &str,
) -> WorkerSnapshot {
    let deadline = Instant::now() + timeout;
    loop {
        let snap = worker.snapshot();
        if predicate(&snap) {
            return snap;
        }
        if Instant::now() >= deadline {
            panic!("{}; last snapshot: {:?}", message, snap);
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn fixture_artifacts() -> &'static FixtureArtifacts {
    FIXTURES.get_or_init(|| FixtureArtifacts {
        alpha_v1: build_fixture_library(
            "tests/fixtures/multi_plugin_alpha_v1/Cargo.toml",
            "multi_plugin_alpha_v1",
        ),
        alpha_next: build_fixture_library(
            "tests/fixtures/multi_plugin_alpha_next/Cargo.toml",
            "multi_plugin_alpha_next",
        ),
        beta_v1: build_fixture_library(
            "tests/fixtures/multi_plugin_beta_v1/Cargo.toml",
            "multi_plugin_beta_v1",
        ),
    })
}

fn build_fixture_library(manifest_rel: &str, crate_name: &str) -> PathBuf {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let manifest_path = crate_root.join(manifest_rel);
    let manifest_dir = manifest_path
        .parent()
        .expect("fixture manifest must have parent dir");

    let status = Command::new(cargo_bin())
        .arg("build")
        .arg("--manifest-path")
        .arg(&manifest_path)
        .current_dir(manifest_dir)
        .status()
        .expect("spawn cargo build for fixture plugin");
    assert!(
        status.success(),
        "fixture build failed: {}",
        manifest_path.display()
    );

    let expected = manifest_dir
        .join("target")
        .join("debug")
        .join(dylib_filename(crate_name));
    if expected.exists() {
        return expected;
    }

    let file_name = dylib_filename(crate_name);
    find_file_recursive(&manifest_dir.join("target").join("debug"), &file_name)
        .unwrap_or_else(|| panic!("cannot locate fixture dylib {}", file_name))
}

fn cargo_bin() -> String {
    std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string())
}

fn dylib_filename(crate_name: &str) -> String {
    let base = crate_name.replace('-', "_");
    match std::env::consts::OS {
        "windows" => format!("{base}.dll"),
        "linux" => format!("lib{base}.so"),
        "macos" => format!("lib{base}.dylib"),
        other => panic!("unsupported test platform: {other}"),
    }
}

fn find_file_recursive(root: &Path, file_name: &str) -> Option<PathBuf> {
    for entry in walkdir::WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        if entry
            .file_name()
            .to_string_lossy()
            .eq_ignore_ascii_case(file_name)
        {
            return Some(entry.path().to_path_buf());
        }
    }
    None
}
