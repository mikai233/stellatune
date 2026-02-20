use wasmtime::component::Resource;

use stellatune_wasm_host_bindings::generated::output_sink_plugin::stellatune::plugin::common as output_sink_common;
use stellatune_wasm_host_bindings::generated::output_sink_plugin::stellatune::plugin::hot_path as output_sink_hot_path;
use stellatune_wasm_host_bindings::generated::output_sink_plugin::stellatune::plugin::sidecar as output_sink_sidecar;

use crate::executor::sidecar_state::SidecarState;
use crate::host::sidecar::{SidecarLaunchSpec, SidecarTransportKind, SidecarTransportOption};

pub(crate) struct OutputSinkStoreData {
    pub(crate) sidecar: SidecarState,
}

impl output_sink_common::Host for OutputSinkStoreData {}
impl output_sink_hot_path::Host for OutputSinkStoreData {}

fn output_sink_plugin_error_internal(
    error: impl std::fmt::Display,
) -> output_sink_sidecar::PluginError {
    output_sink_sidecar::PluginError::Internal(error.to_string())
}

fn output_sink_transport_option_from(
    option: output_sink_sidecar::TransportOption,
) -> SidecarTransportOption {
    SidecarTransportOption {
        kind: match option.kind {
            output_sink_sidecar::TransportKind::Stdio => SidecarTransportKind::Stdio,
            output_sink_sidecar::TransportKind::NamedPipe => SidecarTransportKind::NamedPipe,
            output_sink_sidecar::TransportKind::UnixSocket => SidecarTransportKind::UnixSocket,
            output_sink_sidecar::TransportKind::LoopbackTcp => SidecarTransportKind::LoopbackTcp,
            output_sink_sidecar::TransportKind::SharedMemoryRing => {
                SidecarTransportKind::SharedMemoryRing
            },
        },
        priority: option.priority,
        max_frame_bytes: option.max_frame_bytes,
    }
}

fn output_sink_transport_kind_into(
    kind: SidecarTransportKind,
) -> output_sink_sidecar::TransportKind {
    match kind {
        SidecarTransportKind::Stdio => output_sink_sidecar::TransportKind::Stdio,
        SidecarTransportKind::NamedPipe => output_sink_sidecar::TransportKind::NamedPipe,
        SidecarTransportKind::UnixSocket => output_sink_sidecar::TransportKind::UnixSocket,
        SidecarTransportKind::LoopbackTcp => output_sink_sidecar::TransportKind::LoopbackTcp,
        SidecarTransportKind::SharedMemoryRing => {
            output_sink_sidecar::TransportKind::SharedMemoryRing
        },
    }
}

impl output_sink_sidecar::Host for OutputSinkStoreData {
    fn launch(
        &mut self,
        spec: output_sink_sidecar::LaunchSpec,
    ) -> std::result::Result<Resource<output_sink_sidecar::Process>, output_sink_sidecar::PluginError>
    {
        let process_rep = self
            .sidecar
            .launch(&SidecarLaunchSpec {
                executable: spec.executable,
                args: spec.args,
                preferred_control: spec
                    .preferred_control
                    .into_iter()
                    .map(output_sink_transport_option_from)
                    .collect::<Vec<_>>(),
                preferred_data: spec
                    .preferred_data
                    .into_iter()
                    .map(output_sink_transport_option_from)
                    .collect::<Vec<_>>(),
                env: spec.env,
            })
            .map_err(output_sink_plugin_error_internal)?;
        Ok(Resource::new_own(process_rep))
    }
}

impl output_sink_sidecar::HostProcess for OutputSinkStoreData {
    fn open_control(
        &mut self,
        self_: Resource<output_sink_sidecar::Process>,
    ) -> std::result::Result<Resource<output_sink_sidecar::Channel>, output_sink_sidecar::PluginError>
    {
        let process_rep = self_.rep();
        let channel_rep = self
            .sidecar
            .open_control(process_rep)
            .map_err(output_sink_plugin_error_internal)?;
        Ok(Resource::new_own(channel_rep))
    }

    fn open_data(
        &mut self,
        self_: Resource<output_sink_sidecar::Process>,
        role: String,
        preferred: Vec<output_sink_sidecar::TransportOption>,
    ) -> std::result::Result<Resource<output_sink_sidecar::Channel>, output_sink_sidecar::PluginError>
    {
        let process_rep = self_.rep();
        let preferred = preferred
            .into_iter()
            .map(output_sink_transport_option_from)
            .collect::<Vec<_>>();
        let channel_rep = self
            .sidecar
            .open_data(process_rep, role.trim(), &preferred)
            .map_err(output_sink_plugin_error_internal)?;
        Ok(Resource::new_own(channel_rep))
    }

    fn wait_exit(
        &mut self,
        self_: Resource<output_sink_sidecar::Process>,
        timeout_ms: Option<u32>,
    ) -> std::result::Result<Option<i32>, output_sink_sidecar::PluginError> {
        let process_rep = self_.rep();
        self.sidecar
            .wait_exit(process_rep, timeout_ms)
            .map_err(output_sink_plugin_error_internal)
    }

    fn terminate(
        &mut self,
        self_: Resource<output_sink_sidecar::Process>,
        grace_ms: u32,
    ) -> std::result::Result<(), output_sink_sidecar::PluginError> {
        let process_rep = self_.rep();
        self.sidecar
            .terminate(process_rep, grace_ms)
            .map_err(output_sink_plugin_error_internal)
    }

    fn drop(&mut self, rep: Resource<output_sink_sidecar::Process>) -> wasmtime::Result<()> {
        self.sidecar.drop_process(rep.rep());
        Ok(())
    }
}

impl output_sink_sidecar::HostChannel for OutputSinkStoreData {
    fn transport(
        &mut self,
        self_: Resource<output_sink_sidecar::Channel>,
    ) -> output_sink_sidecar::TransportKind {
        let channel_rep = self_.rep();
        self.sidecar
            .channel_transport(channel_rep)
            .map(output_sink_transport_kind_into)
            .unwrap_or(output_sink_sidecar::TransportKind::Stdio)
    }

    fn write(
        &mut self,
        self_: Resource<output_sink_sidecar::Channel>,
        data: Vec<u8>,
    ) -> std::result::Result<u32, output_sink_sidecar::PluginError> {
        let channel_rep = self_.rep();
        self.sidecar
            .channel_write(channel_rep, &data)
            .map_err(output_sink_plugin_error_internal)
    }

    fn read(
        &mut self,
        self_: Resource<output_sink_sidecar::Channel>,
        max_bytes: u32,
        timeout_ms: Option<u32>,
    ) -> std::result::Result<Vec<u8>, output_sink_sidecar::PluginError> {
        let channel_rep = self_.rep();
        self.sidecar
            .channel_read(channel_rep, max_bytes, timeout_ms)
            .map_err(output_sink_plugin_error_internal)
    }

    fn close(&mut self, self_: Resource<output_sink_sidecar::Channel>) {
        let _ = self.sidecar.channel_close(self_.rep());
    }

    fn drop(&mut self, rep: Resource<output_sink_sidecar::Channel>) -> wasmtime::Result<()> {
        self.sidecar.drop_channel(rep.rep());
        Ok(())
    }
}
