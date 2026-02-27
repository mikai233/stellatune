use std::path::PathBuf;

use wasmtime::component::Resource;
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxView, WasiView};

use stellatune_host_bindings::generated::encoder_plugin::stellatune::plugin::common as encoder_common;
use stellatune_host_bindings::generated::encoder_plugin::stellatune::plugin::sidecar as encoder_sidecar;

use crate::executor::sidecar_state::SidecarState;
use crate::host::sidecar::{
    SidecarLaunchScope, SidecarLaunchSpec, SidecarTransportKind, SidecarTransportOption,
    resolve_sidecar_executable,
};

pub(crate) struct EncoderStoreData {
    pub(crate) sidecar: SidecarState,
    pub(crate) plugin_root: PathBuf,
    pub(crate) wasi_ctx: WasiCtx,
    pub(crate) wasi_table: ResourceTable,
}

impl encoder_common::Host for EncoderStoreData {}

impl WasiView for EncoderStoreData {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi_ctx,
            table: &mut self.wasi_table,
        }
    }
}

fn encoder_plugin_error_internal(error: impl std::fmt::Display) -> encoder_sidecar::PluginError {
    encoder_sidecar::PluginError::Internal(error.to_string())
}

fn encoder_transport_option_from(option: encoder_sidecar::TransportOption) -> SidecarTransportOption {
    SidecarTransportOption {
        kind: match option.kind {
            encoder_sidecar::TransportKind::Stdio => SidecarTransportKind::Stdio,
            encoder_sidecar::TransportKind::NamedPipe => SidecarTransportKind::NamedPipe,
            encoder_sidecar::TransportKind::UnixSocket => SidecarTransportKind::UnixSocket,
            encoder_sidecar::TransportKind::LoopbackTcp => SidecarTransportKind::LoopbackTcp,
            encoder_sidecar::TransportKind::SharedMemoryRing => {
                SidecarTransportKind::SharedMemoryRing
            },
        },
        priority: option.priority,
        max_frame_bytes: option.max_frame_bytes,
    }
}

fn encoder_transport_kind_into(kind: SidecarTransportKind) -> encoder_sidecar::TransportKind {
    match kind {
        SidecarTransportKind::Stdio => encoder_sidecar::TransportKind::Stdio,
        SidecarTransportKind::NamedPipe => encoder_sidecar::TransportKind::NamedPipe,
        SidecarTransportKind::UnixSocket => encoder_sidecar::TransportKind::UnixSocket,
        SidecarTransportKind::LoopbackTcp => encoder_sidecar::TransportKind::LoopbackTcp,
        SidecarTransportKind::SharedMemoryRing => encoder_sidecar::TransportKind::SharedMemoryRing,
    }
}

fn encoder_launch_scope_from(scope: encoder_sidecar::LaunchScope) -> SidecarLaunchScope {
    match scope {
        encoder_sidecar::LaunchScope::Instance => SidecarLaunchScope::Instance,
        encoder_sidecar::LaunchScope::PackageShared => SidecarLaunchScope::Package,
    }
}

impl encoder_sidecar::Host for EncoderStoreData {
    fn lock(
        &mut self,
        name: String,
        timeout_ms: Option<u32>,
    ) -> std::result::Result<Resource<encoder_sidecar::LockGuard>, encoder_sidecar::PluginError>
    {
        let lock_rep = self
            .sidecar
            .lock(name.trim(), timeout_ms)
            .map_err(encoder_plugin_error_internal)?;
        Ok(Resource::new_own(lock_rep))
    }

    fn launch(
        &mut self,
        spec: encoder_sidecar::LaunchSpec,
    ) -> std::result::Result<Resource<encoder_sidecar::Process>, encoder_sidecar::PluginError> {
        let process_rep = self
            .sidecar
            .launch(&SidecarLaunchSpec {
                scope: encoder_launch_scope_from(spec.scope),
                executable: resolve_sidecar_executable(&self.plugin_root, &spec.executable)
                    .map_err(encoder_plugin_error_internal)?,
                args: spec.args,
                preferred_control: spec
                    .preferred_control
                    .into_iter()
                    .map(encoder_transport_option_from)
                    .collect::<Vec<_>>(),
                preferred_data: spec
                    .preferred_data
                    .into_iter()
                    .map(encoder_transport_option_from)
                    .collect::<Vec<_>>(),
                env: spec.env,
            })
            .map_err(encoder_plugin_error_internal)?;
        Ok(Resource::new_own(process_rep))
    }
}

impl encoder_sidecar::HostProcess for EncoderStoreData {
    fn open_control(
        &mut self,
        self_: Resource<encoder_sidecar::Process>,
    ) -> std::result::Result<Resource<encoder_sidecar::Channel>, encoder_sidecar::PluginError> {
        let process_rep = self_.rep();
        let channel_rep = self
            .sidecar
            .open_control(process_rep)
            .map_err(encoder_plugin_error_internal)?;
        Ok(Resource::new_own(channel_rep))
    }

    fn open_data(
        &mut self,
        self_: Resource<encoder_sidecar::Process>,
        role: String,
        preferred: Vec<encoder_sidecar::TransportOption>,
    ) -> std::result::Result<Resource<encoder_sidecar::Channel>, encoder_sidecar::PluginError> {
        let process_rep = self_.rep();
        let preferred = preferred
            .into_iter()
            .map(encoder_transport_option_from)
            .collect::<Vec<_>>();
        let channel_rep = self
            .sidecar
            .open_data(process_rep, role.trim(), &preferred)
            .map_err(encoder_plugin_error_internal)?;
        Ok(Resource::new_own(channel_rep))
    }

    fn wait_exit(
        &mut self,
        self_: Resource<encoder_sidecar::Process>,
        timeout_ms: Option<u32>,
    ) -> std::result::Result<Option<i32>, encoder_sidecar::PluginError> {
        let process_rep = self_.rep();
        self.sidecar
            .wait_exit(process_rep, timeout_ms)
            .map_err(encoder_plugin_error_internal)
    }

    fn terminate(
        &mut self,
        self_: Resource<encoder_sidecar::Process>,
        grace_ms: u32,
    ) -> std::result::Result<(), encoder_sidecar::PluginError> {
        let process_rep = self_.rep();
        self.sidecar
            .terminate(process_rep, grace_ms)
            .map_err(encoder_plugin_error_internal)
    }

    fn drop(&mut self, rep: Resource<encoder_sidecar::Process>) -> wasmtime::Result<()> {
        self.sidecar.drop_process(rep.rep());
        Ok(())
    }
}

impl encoder_sidecar::HostChannel for EncoderStoreData {
    fn transport(
        &mut self,
        self_: Resource<encoder_sidecar::Channel>,
    ) -> encoder_sidecar::TransportKind {
        let channel_rep = self_.rep();
        self.sidecar
            .channel_transport(channel_rep)
            .map(encoder_transport_kind_into)
            .unwrap_or(encoder_sidecar::TransportKind::Stdio)
    }

    fn write(
        &mut self,
        self_: Resource<encoder_sidecar::Channel>,
        data: Vec<u8>,
    ) -> std::result::Result<u32, encoder_sidecar::PluginError> {
        let channel_rep = self_.rep();
        self.sidecar
            .channel_write(channel_rep, &data)
            .map_err(encoder_plugin_error_internal)
    }

    fn read(
        &mut self,
        self_: Resource<encoder_sidecar::Channel>,
        max_bytes: u32,
        timeout_ms: Option<u32>,
    ) -> std::result::Result<Vec<u8>, encoder_sidecar::PluginError> {
        let channel_rep = self_.rep();
        self.sidecar
            .channel_read(channel_rep, max_bytes, timeout_ms)
            .map_err(encoder_plugin_error_internal)
    }

    fn close(&mut self, self_: Resource<encoder_sidecar::Channel>) {
        let _ = self.sidecar.channel_close(self_.rep());
    }

    fn drop(&mut self, rep: Resource<encoder_sidecar::Channel>) -> wasmtime::Result<()> {
        self.sidecar.drop_channel(rep.rep());
        Ok(())
    }
}

impl encoder_sidecar::HostLockGuard for EncoderStoreData {
    fn unlock(&mut self, self_: Resource<encoder_sidecar::LockGuard>) {
        let _ = self.sidecar.unlock(self_.rep());
    }

    fn drop(&mut self, rep: Resource<encoder_sidecar::LockGuard>) -> wasmtime::Result<()> {
        self.sidecar.drop_lock(rep.rep());
        Ok(())
    }
}
