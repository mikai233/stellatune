use wasmtime::component::Resource;

use stellatune_wasm_host_bindings::generated::dsp_plugin::stellatune::plugin::common as dsp_common;
use stellatune_wasm_host_bindings::generated::dsp_plugin::stellatune::plugin::hot_path as dsp_hot_path;
use stellatune_wasm_host_bindings::generated::dsp_plugin::stellatune::plugin::sidecar as dsp_sidecar;

use crate::executor::sidecar_state::SidecarState;
use crate::host::sidecar::{SidecarLaunchSpec, SidecarTransportKind, SidecarTransportOption};

pub(crate) struct DspStoreData {
    pub(crate) sidecar: SidecarState,
}

impl dsp_common::Host for DspStoreData {}
impl dsp_hot_path::Host for DspStoreData {}

fn dsp_plugin_error_internal(error: impl std::fmt::Display) -> dsp_sidecar::PluginError {
    dsp_sidecar::PluginError::Internal(error.to_string())
}

fn dsp_transport_option_from(option: dsp_sidecar::TransportOption) -> SidecarTransportOption {
    SidecarTransportOption {
        kind: match option.kind {
            dsp_sidecar::TransportKind::Stdio => SidecarTransportKind::Stdio,
            dsp_sidecar::TransportKind::NamedPipe => SidecarTransportKind::NamedPipe,
            dsp_sidecar::TransportKind::UnixSocket => SidecarTransportKind::UnixSocket,
            dsp_sidecar::TransportKind::LoopbackTcp => SidecarTransportKind::LoopbackTcp,
            dsp_sidecar::TransportKind::SharedMemoryRing => SidecarTransportKind::SharedMemoryRing,
        },
        priority: option.priority,
        max_frame_bytes: option.max_frame_bytes,
    }
}

fn dsp_transport_kind_into(kind: SidecarTransportKind) -> dsp_sidecar::TransportKind {
    match kind {
        SidecarTransportKind::Stdio => dsp_sidecar::TransportKind::Stdio,
        SidecarTransportKind::NamedPipe => dsp_sidecar::TransportKind::NamedPipe,
        SidecarTransportKind::UnixSocket => dsp_sidecar::TransportKind::UnixSocket,
        SidecarTransportKind::LoopbackTcp => dsp_sidecar::TransportKind::LoopbackTcp,
        SidecarTransportKind::SharedMemoryRing => dsp_sidecar::TransportKind::SharedMemoryRing,
    }
}

impl dsp_sidecar::Host for DspStoreData {
    fn launch(
        &mut self,
        spec: dsp_sidecar::LaunchSpec,
    ) -> std::result::Result<Resource<dsp_sidecar::Process>, dsp_sidecar::PluginError> {
        let process_rep = self
            .sidecar
            .launch(&SidecarLaunchSpec {
                executable: spec.executable,
                args: spec.args,
                preferred_control: spec
                    .preferred_control
                    .into_iter()
                    .map(dsp_transport_option_from)
                    .collect::<Vec<_>>(),
                preferred_data: spec
                    .preferred_data
                    .into_iter()
                    .map(dsp_transport_option_from)
                    .collect::<Vec<_>>(),
                env: spec.env,
            })
            .map_err(dsp_plugin_error_internal)?;
        Ok(Resource::new_own(process_rep))
    }
}

impl dsp_sidecar::HostProcess for DspStoreData {
    fn open_control(
        &mut self,
        self_: Resource<dsp_sidecar::Process>,
    ) -> std::result::Result<Resource<dsp_sidecar::Channel>, dsp_sidecar::PluginError> {
        let process_rep = self_.rep();
        let channel_rep = self
            .sidecar
            .open_control(process_rep)
            .map_err(dsp_plugin_error_internal)?;
        Ok(Resource::new_own(channel_rep))
    }

    fn open_data(
        &mut self,
        self_: Resource<dsp_sidecar::Process>,
        role: String,
        preferred: Vec<dsp_sidecar::TransportOption>,
    ) -> std::result::Result<Resource<dsp_sidecar::Channel>, dsp_sidecar::PluginError> {
        let process_rep = self_.rep();
        let preferred = preferred
            .into_iter()
            .map(dsp_transport_option_from)
            .collect::<Vec<_>>();
        let channel_rep = self
            .sidecar
            .open_data(process_rep, role.trim(), &preferred)
            .map_err(dsp_plugin_error_internal)?;
        Ok(Resource::new_own(channel_rep))
    }

    fn wait_exit(
        &mut self,
        self_: Resource<dsp_sidecar::Process>,
        timeout_ms: Option<u32>,
    ) -> std::result::Result<Option<i32>, dsp_sidecar::PluginError> {
        let process_rep = self_.rep();
        self.sidecar
            .wait_exit(process_rep, timeout_ms)
            .map_err(dsp_plugin_error_internal)
    }

    fn terminate(
        &mut self,
        self_: Resource<dsp_sidecar::Process>,
        grace_ms: u32,
    ) -> std::result::Result<(), dsp_sidecar::PluginError> {
        let process_rep = self_.rep();
        self.sidecar
            .terminate(process_rep, grace_ms)
            .map_err(dsp_plugin_error_internal)
    }

    fn drop(&mut self, rep: Resource<dsp_sidecar::Process>) -> wasmtime::Result<()> {
        self.sidecar.drop_process(rep.rep());
        Ok(())
    }
}

impl dsp_sidecar::HostChannel for DspStoreData {
    fn transport(&mut self, self_: Resource<dsp_sidecar::Channel>) -> dsp_sidecar::TransportKind {
        let channel_rep = self_.rep();
        self.sidecar
            .channel_transport(channel_rep)
            .map(dsp_transport_kind_into)
            .unwrap_or(dsp_sidecar::TransportKind::Stdio)
    }

    fn write(
        &mut self,
        self_: Resource<dsp_sidecar::Channel>,
        data: Vec<u8>,
    ) -> std::result::Result<u32, dsp_sidecar::PluginError> {
        let channel_rep = self_.rep();
        self.sidecar
            .channel_write(channel_rep, &data)
            .map_err(dsp_plugin_error_internal)
    }

    fn read(
        &mut self,
        self_: Resource<dsp_sidecar::Channel>,
        max_bytes: u32,
        timeout_ms: Option<u32>,
    ) -> std::result::Result<Vec<u8>, dsp_sidecar::PluginError> {
        let channel_rep = self_.rep();
        self.sidecar
            .channel_read(channel_rep, max_bytes, timeout_ms)
            .map_err(dsp_plugin_error_internal)
    }

    fn close(&mut self, self_: Resource<dsp_sidecar::Channel>) {
        let _ = self.sidecar.channel_close(self_.rep());
    }

    fn drop(&mut self, rep: Resource<dsp_sidecar::Channel>) -> wasmtime::Result<()> {
        self.sidecar.drop_channel(rep.rep());
        Ok(())
    }
}
