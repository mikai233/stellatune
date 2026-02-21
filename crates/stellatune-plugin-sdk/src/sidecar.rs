use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::error::{SdkError, SdkResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportKind {
    Stdio,
    NamedPipe,
    UnixSocket,
    LoopbackTcp,
    SharedMemoryRing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportOption {
    pub kind: TransportKind,
    pub priority: u8,
    pub max_frame_bytes: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidecarLaunchSpec {
    pub executable: String,
    pub args: Vec<String>,
    pub preferred_control: Vec<TransportOption>,
    pub preferred_data: Vec<TransportOption>,
    pub env: Vec<(String, String)>,
}

pub trait SidecarChannel: Send {
    fn transport(&self) -> TransportKind;
    fn write(&mut self, data: &[u8]) -> SdkResult<u32>;
    fn read(&mut self, max_bytes: u32, timeout_ms: Option<u32>) -> SdkResult<Vec<u8>>;
    fn close(&mut self) -> SdkResult<()> {
        Ok(())
    }
}

pub trait SidecarChannelExt: SidecarChannel {
    fn write_json<T: Serialize>(&mut self, value: &T) -> SdkResult<u32> {
        let payload = serde_json::to_vec(value)
            .map_err(|error| SdkError::invalid_arg(format!("serialize sidecar JSON: {error}")))?;
        self.write(&payload)
    }

    fn read_json<T: DeserializeOwned>(
        &mut self,
        max_bytes: u32,
        timeout_ms: Option<u32>,
    ) -> SdkResult<T> {
        let payload = self.read(max_bytes, timeout_ms)?;
        serde_json::from_slice::<T>(&payload).map_err(|error| {
            SdkError::invalid_arg(format!("deserialize sidecar JSON payload: {error}"))
        })
    }
}

impl<T: SidecarChannel + ?Sized> SidecarChannelExt for T {}

pub trait SidecarProcess: Send {
    type Channel: SidecarChannel;

    fn open_control(&mut self) -> SdkResult<Self::Channel>;
    fn open_data(&mut self, role: &str, preferred: &[TransportOption]) -> SdkResult<Self::Channel>;
    fn wait_exit(&mut self, timeout_ms: Option<u32>) -> SdkResult<Option<i32>>;
    fn terminate(&mut self, grace_ms: u32) -> SdkResult<()>;
}

pub trait SidecarProcessExt: SidecarProcess {
    fn terminate_and_wait(&mut self, grace_ms: u32, wait_timeout_ms: Option<u32>) -> SdkResult<()> {
        self.terminate(grace_ms)?;
        let _ = self.wait_exit(wait_timeout_ms)?;
        Ok(())
    }
}

impl<T: SidecarProcess + ?Sized> SidecarProcessExt for T {}

pub trait SidecarClient {
    type Process: SidecarProcess;
    fn launch(&mut self, spec: &SidecarLaunchSpec) -> SdkResult<Self::Process>;
}

pub fn ordered_transport_options(options: &[TransportOption]) -> Vec<TransportOption> {
    let mut out = options.to_vec();
    out.sort_by(|left, right| {
        right
            .priority
            .cmp(&left.priority)
            .then_with(|| (left.kind as u8).cmp(&(right.kind as u8)))
    });
    out
}
