use std::collections::BTreeMap;
use std::sync::Arc;

use crate::error::Result;

use crate::host::sidecar::{
    SidecarChannelHandle, SidecarHost, SidecarLaunchSpec, SidecarProcessHandle,
    SidecarTransportKind, SidecarTransportOption,
};

pub(crate) struct SidecarState {
    host: Arc<dyn SidecarHost>,
    next_process_rep: u32,
    next_channel_rep: u32,
    processes: BTreeMap<u32, Box<dyn SidecarProcessHandle>>,
    channels: BTreeMap<u32, Box<dyn SidecarChannelHandle>>,
}

impl SidecarState {
    pub(crate) fn new(host: Arc<dyn SidecarHost>) -> Self {
        Self {
            host,
            next_process_rep: 1,
            next_channel_rep: 1,
            processes: BTreeMap::new(),
            channels: BTreeMap::new(),
        }
    }

    pub(crate) fn launch(&mut self, spec: &SidecarLaunchSpec) -> Result<u32> {
        let process = self.host.launch(spec)?;
        let process_rep = self.alloc_process_rep();
        self.processes.insert(process_rep, process);
        Ok(process_rep)
    }

    pub(crate) fn open_control(&mut self, process_rep: u32) -> Result<u32> {
        let process = self
            .processes
            .get_mut(&process_rep)
            .ok_or_else(|| crate::op_error!("sidecar process handle `{process_rep}` not found"))?;
        let channel = process.open_control()?;
        let channel_rep = self.alloc_channel_rep();
        self.channels.insert(channel_rep, channel);
        Ok(channel_rep)
    }

    pub(crate) fn open_data(
        &mut self,
        process_rep: u32,
        role: &str,
        preferred: &[SidecarTransportOption],
    ) -> Result<u32> {
        let process = self
            .processes
            .get_mut(&process_rep)
            .ok_or_else(|| crate::op_error!("sidecar process handle `{process_rep}` not found"))?;
        let channel = process.open_data(role, preferred)?;
        let channel_rep = self.alloc_channel_rep();
        self.channels.insert(channel_rep, channel);
        Ok(channel_rep)
    }

    pub(crate) fn wait_exit(
        &mut self,
        process_rep: u32,
        timeout_ms: Option<u32>,
    ) -> Result<Option<i32>> {
        let process = self
            .processes
            .get_mut(&process_rep)
            .ok_or_else(|| crate::op_error!("sidecar process handle `{process_rep}` not found"))?;
        process.wait_exit(timeout_ms)
    }

    pub(crate) fn terminate(&mut self, process_rep: u32, grace_ms: u32) -> Result<()> {
        let process = self
            .processes
            .get_mut(&process_rep)
            .ok_or_else(|| crate::op_error!("sidecar process handle `{process_rep}` not found"))?;
        process.terminate(grace_ms)
    }

    pub(crate) fn channel_transport(&mut self, channel_rep: u32) -> Result<SidecarTransportKind> {
        let channel = self
            .channels
            .get_mut(&channel_rep)
            .ok_or_else(|| crate::op_error!("sidecar channel handle `{channel_rep}` not found"))?;
        Ok(channel.transport())
    }

    pub(crate) fn channel_write(&mut self, channel_rep: u32, data: &[u8]) -> Result<u32> {
        let channel = self
            .channels
            .get_mut(&channel_rep)
            .ok_or_else(|| crate::op_error!("sidecar channel handle `{channel_rep}` not found"))?;
        channel.write(data)
    }

    pub(crate) fn channel_read(
        &mut self,
        channel_rep: u32,
        max_bytes: u32,
        timeout_ms: Option<u32>,
    ) -> Result<Vec<u8>> {
        let channel = self
            .channels
            .get_mut(&channel_rep)
            .ok_or_else(|| crate::op_error!("sidecar channel handle `{channel_rep}` not found"))?;
        channel.read(max_bytes, timeout_ms)
    }

    pub(crate) fn channel_close(&mut self, channel_rep: u32) -> Result<()> {
        let channel = self
            .channels
            .get_mut(&channel_rep)
            .ok_or_else(|| crate::op_error!("sidecar channel handle `{channel_rep}` not found"))?;
        channel.close();
        Ok(())
    }

    pub(crate) fn drop_process(&mut self, process_rep: u32) {
        if let Some(mut process) = self.processes.remove(&process_rep) {
            let _ = process.terminate(0);
        }
    }

    pub(crate) fn drop_channel(&mut self, channel_rep: u32) {
        if let Some(mut channel) = self.channels.remove(&channel_rep) {
            channel.close();
        }
    }

    fn alloc_process_rep(&mut self) -> u32 {
        let rep = self.next_process_rep;
        self.next_process_rep = self.next_process_rep.saturating_add(1);
        if self.next_process_rep == 0 {
            self.next_process_rep = 1;
        }
        rep
    }

    fn alloc_channel_rep(&mut self) -> u32 {
        let rep = self.next_channel_rep;
        self.next_channel_rep = self.next_channel_rep.saturating_add(1);
        if self.next_channel_rep == 0 {
            self.next_channel_rep = 1;
        }
        rep
    }
}

impl Drop for SidecarState {
    fn drop(&mut self) {
        for (_, mut channel) in std::mem::take(&mut self.channels) {
            channel.close();
        }
        for (_, mut process) in std::mem::take(&mut self.processes) {
            let _ = process.terminate(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::error::Result;
    use crate::executor::sidecar_state::SidecarState;
    use crate::host::sidecar::{
        SidecarChannelHandle, SidecarHost, SidecarLaunchSpec, SidecarProcessHandle,
        SidecarTransportKind, SidecarTransportOption,
    };

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
        SidecarState::new(Arc::new(FakeHost))
    }

    #[test]
    fn launch_and_io_path_works() {
        let mut state = create_state();
        let process_rep = state
            .launch(&SidecarLaunchSpec {
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
}
