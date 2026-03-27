use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::error::Result;
use crate::host::sidecar::types::{
    SidecarChannelHandle, SidecarHost, SidecarLaunchScope, SidecarLaunchSpec, SidecarProcessHandle,
    SidecarTransportKind, SidecarTransportOption,
};

use super::registry::PackageSidecarRegistry;
use super::state::SidecarState;

struct FakeChannel {
    kind: SidecarTransportKind,
    closed: bool,
}

impl SidecarChannelHandle for FakeChannel {
    fn transport(&self) -> SidecarTransportKind {
        self.kind
    }

    fn write(&mut self, data: &[u8]) -> Result<u32> {
        Ok(data.len() as u32)
    }

    fn read(&mut self, max_bytes: u32, _timeout_ms: Option<u32>) -> Result<Vec<u8>> {
        let size = max_bytes.min(4) as usize;
        Ok(vec![7; size])
    }

    fn close(&mut self) {
        self.closed = true;
    }
}

struct FakeProcess;

impl SidecarProcessHandle for FakeProcess {
    fn open_control(&mut self) -> Result<Box<dyn SidecarChannelHandle>> {
        Ok(Box::new(FakeChannel {
            kind: SidecarTransportKind::Stdio,
            closed: false,
        }))
    }

    fn open_data(
        &mut self,
        _role: &str,
        preferred: &[SidecarTransportOption],
    ) -> Result<Box<dyn SidecarChannelHandle>> {
        let kind = preferred
            .first()
            .map(|option| option.kind)
            .unwrap_or(SidecarTransportKind::LoopbackTcp);
        Ok(Box::new(FakeChannel {
            kind,
            closed: false,
        }))
    }

    fn wait_exit(&mut self, _timeout_ms: Option<u32>) -> Result<Option<i32>> {
        Ok(Some(0))
    }

    fn terminate(&mut self, _grace_ms: u32) -> Result<()> {
        Ok(())
    }
}

struct FakeHost;

impl SidecarHost for FakeHost {
    fn launch(&self, _spec: &SidecarLaunchSpec) -> Result<Box<dyn SidecarProcessHandle>> {
        Ok(Box::new(FakeProcess))
    }
}

fn create_state() -> SidecarState {
    let registry = PackageSidecarRegistry::new(Arc::new(FakeHost));
    SidecarState::new("dev.stellatune.test".to_string(), registry)
}

#[test]
fn launch_and_io_path_works() {
    let mut state = create_state();
    let process_rep = state
        .launch(&SidecarLaunchSpec {
            scope: SidecarLaunchScope::Package,
            executable: "demo.exe".to_string(),
            args: Vec::new(),
            preferred_control: Vec::new(),
            preferred_data: Vec::new(),
            env: Vec::new(),
        })
        .expect("launch");
    let channel_rep = state.open_control(process_rep).expect("open control");
    assert_eq!(
        state.channel_transport(channel_rep).expect("transport"),
        SidecarTransportKind::Stdio
    );
    assert_eq!(
        state.channel_write(channel_rep, &[1, 2, 3]).expect("write"),
        3
    );
    assert_eq!(
        state.channel_read(channel_rep, 8, None).expect("read"),
        vec![7, 7, 7, 7]
    );
    state.channel_close(channel_rep).expect("close");
    assert_eq!(
        state.wait_exit(process_rep, Some(100)).expect("wait"),
        Some(0)
    );
    state.terminate(process_rep, 50).expect("terminate");
}

#[test]
fn data_channel_transport_follows_preferred_kind() {
    let kinds = [
        SidecarTransportKind::Stdio,
        SidecarTransportKind::NamedPipe,
        SidecarTransportKind::UnixSocket,
        SidecarTransportKind::LoopbackTcp,
        SidecarTransportKind::SharedMemoryRing,
    ];
    for kind in kinds {
        let mut state = create_state();
        let process_rep = state
            .launch(&SidecarLaunchSpec {
                scope: SidecarLaunchScope::Package,
                executable: "demo.exe".to_string(),
                args: Vec::new(),
                preferred_control: Vec::new(),
                preferred_data: Vec::new(),
                env: Vec::new(),
            })
            .expect("launch");
        let channel_rep = state
            .open_data(
                process_rep,
                "sink",
                &[SidecarTransportOption {
                    kind,
                    priority: 1,
                    max_frame_bytes: None,
                }],
            )
            .expect("open data");
        assert_eq!(
            state.channel_transport(channel_rep).expect("transport"),
            kind
        );
    }
}

#[test]
fn missing_handles_return_error() {
    let mut state = create_state();
    let error = state
        .open_control(999)
        .expect_err("missing process should fail");
    assert!(error.to_string().contains("not found"));
    let error = state
        .channel_read(999, 16, None)
        .expect_err("missing channel should fail");
    assert!(error.to_string().contains("not found"));
}

struct CountingHost {
    launches: Arc<AtomicUsize>,
}

impl SidecarHost for CountingHost {
    fn launch(&self, _spec: &SidecarLaunchSpec) -> Result<Box<dyn SidecarProcessHandle>> {
        self.launches.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(FakeProcess))
    }
}

#[test]
fn share_sidecar_process_within_same_plugin() {
    let launches = Arc::new(AtomicUsize::new(0));
    let registry = PackageSidecarRegistry::new(Arc::new(CountingHost {
        launches: launches.clone(),
    }));
    let mut first = SidecarState::new("dev.stellatune.shared".to_string(), registry.clone());
    let mut second = SidecarState::new("dev.stellatune.shared".to_string(), registry);

    let spec = SidecarLaunchSpec {
        scope: SidecarLaunchScope::Package,
        executable: "demo.exe".to_string(),
        args: vec!["--api".to_string()],
        preferred_control: Vec::new(),
        preferred_data: Vec::new(),
        env: vec![("TOKEN".to_string(), "abc".to_string())],
    };
    let first_rep = first.launch(&spec).expect("first launch");
    let second_rep = second.launch(&spec).expect("second launch");

    assert_eq!(launches.load(Ordering::SeqCst), 1);

    first.drop_process(first_rep);
    assert_eq!(launches.load(Ordering::SeqCst), 1);
    second.drop_process(second_rep);
}

#[test]
fn instance_scope_does_not_share_process() {
    let launches = Arc::new(AtomicUsize::new(0));
    let registry = PackageSidecarRegistry::new(Arc::new(CountingHost {
        launches: launches.clone(),
    }));
    let mut first = SidecarState::new("dev.stellatune.instance".to_string(), registry.clone());
    let mut second = SidecarState::new("dev.stellatune.instance".to_string(), registry);

    let spec = SidecarLaunchSpec {
        scope: SidecarLaunchScope::Instance,
        executable: "demo.exe".to_string(),
        args: vec!["--api".to_string()],
        preferred_control: Vec::new(),
        preferred_data: Vec::new(),
        env: Vec::new(),
    };
    let first_rep = first.launch(&spec).expect("first launch");
    let second_rep = second.launch(&spec).expect("second launch");

    assert_eq!(launches.load(Ordering::SeqCst), 2);

    first.drop_process(first_rep);
    second.drop_process(second_rep);
}

#[test]
fn lock_is_serialized_per_plugin_and_name() {
    let registry = PackageSidecarRegistry::new(Arc::new(FakeHost));
    let mut first = SidecarState::new("dev.stellatune.shared-lock".to_string(), registry.clone());
    let mut second = SidecarState::new("dev.stellatune.shared-lock".to_string(), registry);

    let first_lock = first.lock("asio-control", Some(100)).expect("first lock");
    let error = second
        .lock("asio-control", Some(20))
        .expect_err("second lock should timeout while first lock is held");
    assert!(error.to_string().contains("timed out"));

    first.unlock(first_lock).expect("first unlock");

    let second_lock = second
        .lock("asio-control", Some(100))
        .expect("second lock after release");
    second.unlock(second_lock).expect("second unlock");
}
