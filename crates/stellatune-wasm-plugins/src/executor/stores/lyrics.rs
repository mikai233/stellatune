use std::path::PathBuf;
use std::sync::Arc;

use wasmtime::component::Resource;
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxView, WasiView};

use stellatune_wasm_host_bindings::generated::lyrics_plugin::stellatune::plugin::common as lyrics_common;
use stellatune_wasm_host_bindings::generated::lyrics_plugin::stellatune::plugin::http_client as lyrics_http_client;
use stellatune_wasm_host_bindings::generated::lyrics_plugin::stellatune::plugin::sidecar as lyrics_sidecar;

use crate::executor::sidecar_state::SidecarState;
use crate::host::http::HttpClientHost;
use crate::host::sidecar::{
    SidecarLaunchSpec, SidecarTransportKind, SidecarTransportOption, resolve_sidecar_executable,
};

pub(crate) struct LyricsStoreData {
    pub(crate) http_client: Arc<dyn HttpClientHost>,
    pub(crate) sidecar: SidecarState,
    pub(crate) plugin_root: PathBuf,
    pub(crate) wasi_ctx: WasiCtx,
    pub(crate) wasi_table: ResourceTable,
}

impl lyrics_common::Host for LyricsStoreData {}

impl WasiView for LyricsStoreData {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi_ctx,
            table: &mut self.wasi_table,
        }
    }
}

impl lyrics_http_client::Host for LyricsStoreData {
    fn fetch_json(
        &mut self,
        url: String,
    ) -> std::result::Result<String, lyrics_http_client::PluginError> {
        self.http_client
            .fetch_json(&url)
            .map_err(|error| lyrics_http_client::PluginError::Internal(error.to_string()))
    }
}

fn lyrics_plugin_error_internal(error: impl std::fmt::Display) -> lyrics_sidecar::PluginError {
    lyrics_sidecar::PluginError::Internal(error.to_string())
}

fn lyrics_transport_option_from(option: lyrics_sidecar::TransportOption) -> SidecarTransportOption {
    SidecarTransportOption {
        kind: match option.kind {
            lyrics_sidecar::TransportKind::Stdio => SidecarTransportKind::Stdio,
            lyrics_sidecar::TransportKind::NamedPipe => SidecarTransportKind::NamedPipe,
            lyrics_sidecar::TransportKind::UnixSocket => SidecarTransportKind::UnixSocket,
            lyrics_sidecar::TransportKind::LoopbackTcp => SidecarTransportKind::LoopbackTcp,
            lyrics_sidecar::TransportKind::SharedMemoryRing => {
                SidecarTransportKind::SharedMemoryRing
            },
        },
        priority: option.priority,
        max_frame_bytes: option.max_frame_bytes,
    }
}

fn lyrics_transport_kind_into(kind: SidecarTransportKind) -> lyrics_sidecar::TransportKind {
    match kind {
        SidecarTransportKind::Stdio => lyrics_sidecar::TransportKind::Stdio,
        SidecarTransportKind::NamedPipe => lyrics_sidecar::TransportKind::NamedPipe,
        SidecarTransportKind::UnixSocket => lyrics_sidecar::TransportKind::UnixSocket,
        SidecarTransportKind::LoopbackTcp => lyrics_sidecar::TransportKind::LoopbackTcp,
        SidecarTransportKind::SharedMemoryRing => lyrics_sidecar::TransportKind::SharedMemoryRing,
    }
}

impl lyrics_sidecar::Host for LyricsStoreData {
    fn launch(
        &mut self,
        spec: lyrics_sidecar::LaunchSpec,
    ) -> std::result::Result<Resource<lyrics_sidecar::Process>, lyrics_sidecar::PluginError> {
        let process_rep = self
            .sidecar
            .launch(&SidecarLaunchSpec {
                executable: resolve_sidecar_executable(&self.plugin_root, &spec.executable)
                    .map_err(lyrics_plugin_error_internal)?,
                args: spec.args,
                preferred_control: spec
                    .preferred_control
                    .into_iter()
                    .map(lyrics_transport_option_from)
                    .collect::<Vec<_>>(),
                preferred_data: spec
                    .preferred_data
                    .into_iter()
                    .map(lyrics_transport_option_from)
                    .collect::<Vec<_>>(),
                env: spec.env,
            })
            .map_err(lyrics_plugin_error_internal)?;
        Ok(Resource::new_own(process_rep))
    }
}

impl lyrics_sidecar::HostProcess for LyricsStoreData {
    fn open_control(
        &mut self,
        self_: Resource<lyrics_sidecar::Process>,
    ) -> std::result::Result<Resource<lyrics_sidecar::Channel>, lyrics_sidecar::PluginError> {
        let process_rep = self_.rep();
        let channel_rep = self
            .sidecar
            .open_control(process_rep)
            .map_err(lyrics_plugin_error_internal)?;
        Ok(Resource::new_own(channel_rep))
    }

    fn open_data(
        &mut self,
        self_: Resource<lyrics_sidecar::Process>,
        role: String,
        preferred: Vec<lyrics_sidecar::TransportOption>,
    ) -> std::result::Result<Resource<lyrics_sidecar::Channel>, lyrics_sidecar::PluginError> {
        let process_rep = self_.rep();
        let preferred = preferred
            .into_iter()
            .map(lyrics_transport_option_from)
            .collect::<Vec<_>>();
        let channel_rep = self
            .sidecar
            .open_data(process_rep, role.trim(), &preferred)
            .map_err(lyrics_plugin_error_internal)?;
        Ok(Resource::new_own(channel_rep))
    }

    fn wait_exit(
        &mut self,
        self_: Resource<lyrics_sidecar::Process>,
        timeout_ms: Option<u32>,
    ) -> std::result::Result<Option<i32>, lyrics_sidecar::PluginError> {
        let process_rep = self_.rep();
        self.sidecar
            .wait_exit(process_rep, timeout_ms)
            .map_err(lyrics_plugin_error_internal)
    }

    fn terminate(
        &mut self,
        self_: Resource<lyrics_sidecar::Process>,
        grace_ms: u32,
    ) -> std::result::Result<(), lyrics_sidecar::PluginError> {
        let process_rep = self_.rep();
        self.sidecar
            .terminate(process_rep, grace_ms)
            .map_err(lyrics_plugin_error_internal)
    }

    fn drop(&mut self, rep: Resource<lyrics_sidecar::Process>) -> wasmtime::Result<()> {
        self.sidecar.drop_process(rep.rep());
        Ok(())
    }
}

impl lyrics_sidecar::HostChannel for LyricsStoreData {
    fn transport(
        &mut self,
        self_: Resource<lyrics_sidecar::Channel>,
    ) -> lyrics_sidecar::TransportKind {
        let channel_rep = self_.rep();
        self.sidecar
            .channel_transport(channel_rep)
            .map(lyrics_transport_kind_into)
            .unwrap_or(lyrics_sidecar::TransportKind::Stdio)
    }

    fn write(
        &mut self,
        self_: Resource<lyrics_sidecar::Channel>,
        data: Vec<u8>,
    ) -> std::result::Result<u32, lyrics_sidecar::PluginError> {
        let channel_rep = self_.rep();
        self.sidecar
            .channel_write(channel_rep, &data)
            .map_err(lyrics_plugin_error_internal)
    }

    fn read(
        &mut self,
        self_: Resource<lyrics_sidecar::Channel>,
        max_bytes: u32,
        timeout_ms: Option<u32>,
    ) -> std::result::Result<Vec<u8>, lyrics_sidecar::PluginError> {
        let channel_rep = self_.rep();
        self.sidecar
            .channel_read(channel_rep, max_bytes, timeout_ms)
            .map_err(lyrics_plugin_error_internal)
    }

    fn close(&mut self, self_: Resource<lyrics_sidecar::Channel>) {
        let _ = self.sidecar.channel_close(self_.rep());
    }

    fn drop(&mut self, rep: Resource<lyrics_sidecar::Channel>) -> wasmtime::Result<()> {
        self.sidecar.drop_channel(rep.rep());
        Ok(())
    }
}
