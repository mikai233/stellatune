use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use wasmtime::component::Resource;
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxView, WasiView};

use stellatune_host_bindings::generated::source_plugin::stellatune::plugin::common as source_common;
use stellatune_host_bindings::generated::source_plugin::stellatune::plugin::host_stream as source_host_stream;
use stellatune_host_bindings::generated::source_plugin::stellatune::plugin::sidecar as source_sidecar;

use crate::executor::sidecar_state::SidecarState;
use crate::host::sidecar::{
    SidecarLaunchSpec, SidecarTransportKind, SidecarTransportOption, resolve_sidecar_executable,
};
use crate::host::stream::{
    HostStreamHandle, HostStreamOpenRequest, HostStreamService, StreamHeader, StreamHttpMethod,
    StreamOpenKind, StreamSeekWhence,
};

pub(crate) struct SourceStoreData {
    pub(crate) stream_service: Arc<dyn HostStreamService>,
    pub(crate) next_rep: u32,
    pub(crate) streams: BTreeMap<u32, Box<dyn HostStreamHandle>>,
    pub(crate) sidecar: SidecarState,
    pub(crate) plugin_root: PathBuf,
    pub(crate) wasi_ctx: WasiCtx,
    pub(crate) wasi_table: ResourceTable,
}

impl SourceStoreData {
    pub(crate) fn alloc_rep(&mut self) -> u32 {
        let rep = self.next_rep;
        self.next_rep = self.next_rep.saturating_add(1);
        if self.next_rep == 0 {
            self.next_rep = 1;
        }
        rep
    }
}

impl source_common::Host for SourceStoreData {}

impl WasiView for SourceStoreData {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi_ctx,
            table: &mut self.wasi_table,
        }
    }
}

impl source_host_stream::HostHostStreamHandle for SourceStoreData {
    fn read(
        &mut self,
        self_: Resource<source_host_stream::HostStreamHandle>,
        max_bytes: u32,
    ) -> std::result::Result<Vec<u8>, source_host_stream::PluginError> {
        let Some(stream) = self.streams.get_mut(&self_.rep()) else {
            return Err(source_host_stream::PluginError::NotFound(format!(
                "host stream handle `{}` not found",
                self_.rep()
            )));
        };
        stream
            .read(max_bytes)
            .map_err(|error| source_host_stream::PluginError::Internal(error.to_string()))
    }

    fn seek(
        &mut self,
        self_: Resource<source_host_stream::HostStreamHandle>,
        offset: i64,
        whence: source_host_stream::SeekWhence,
    ) -> std::result::Result<u64, source_host_stream::PluginError> {
        let Some(stream) = self.streams.get_mut(&self_.rep()) else {
            return Err(source_host_stream::PluginError::NotFound(format!(
                "host stream handle `{}` not found",
                self_.rep()
            )));
        };
        let whence = match whence {
            source_host_stream::SeekWhence::Start => StreamSeekWhence::Start,
            source_host_stream::SeekWhence::Current => StreamSeekWhence::Current,
            source_host_stream::SeekWhence::End => StreamSeekWhence::End,
        };
        stream
            .seek(offset, whence)
            .map_err(|error| source_host_stream::PluginError::Internal(error.to_string()))
    }

    fn tell(
        &mut self,
        self_: Resource<source_host_stream::HostStreamHandle>,
    ) -> std::result::Result<u64, source_host_stream::PluginError> {
        let Some(stream) = self.streams.get_mut(&self_.rep()) else {
            return Err(source_host_stream::PluginError::NotFound(format!(
                "host stream handle `{}` not found",
                self_.rep()
            )));
        };
        stream
            .tell()
            .map_err(|error| source_host_stream::PluginError::Internal(error.to_string()))
    }

    fn size(
        &mut self,
        self_: Resource<source_host_stream::HostStreamHandle>,
    ) -> std::result::Result<u64, source_host_stream::PluginError> {
        let Some(stream) = self.streams.get_mut(&self_.rep()) else {
            return Err(source_host_stream::PluginError::NotFound(format!(
                "host stream handle `{}` not found",
                self_.rep()
            )));
        };
        stream
            .size()
            .map_err(|error| source_host_stream::PluginError::Internal(error.to_string()))
    }

    fn close(&mut self, self_: Resource<source_host_stream::HostStreamHandle>) {
        if let Some(mut stream) = self.streams.remove(&self_.rep()) {
            stream.close();
        }
    }

    fn drop(
        &mut self,
        rep: Resource<source_host_stream::HostStreamHandle>,
    ) -> wasmtime::Result<()> {
        self.streams.remove(&rep.rep());
        Ok(())
    }
}

impl source_host_stream::Host for SourceStoreData {
    fn open(
        &mut self,
        request: source_host_stream::OpenRequest,
    ) -> std::result::Result<
        Resource<source_host_stream::HostStreamHandle>,
        source_host_stream::PluginError,
    > {
        let kind = match request.kind {
            source_host_stream::StreamOpenKind::File => StreamOpenKind::File,
            source_host_stream::StreamOpenKind::Http => StreamOpenKind::Http,
            source_host_stream::StreamOpenKind::Tcp => StreamOpenKind::Tcp,
            source_host_stream::StreamOpenKind::Udp => StreamOpenKind::Udp,
        };
        let method = request.method.map(|method| match method {
            source_host_stream::HttpMethod::Get => StreamHttpMethod::Get,
            source_host_stream::HttpMethod::Post => StreamHttpMethod::Post,
            source_host_stream::HttpMethod::Put => StreamHttpMethod::Put,
            source_host_stream::HttpMethod::Delete => StreamHttpMethod::Delete,
            source_host_stream::HttpMethod::Head => StreamHttpMethod::Head,
            source_host_stream::HttpMethod::Patch => StreamHttpMethod::Patch,
        });
        let request = HostStreamOpenRequest {
            kind,
            target: request.target,
            method,
            headers: request
                .headers
                .into_iter()
                .map(|header| StreamHeader {
                    name: header.name,
                    value: header.value,
                })
                .collect::<Vec<_>>(),
            body: request.body,
            connect_timeout_ms: request.connect_timeout_ms,
            read_timeout_ms: request.read_timeout_ms,
        };
        let stream = self
            .stream_service
            .open(&request)
            .map_err(|error| source_host_stream::PluginError::Internal(error.to_string()))?;
        let rep = self.alloc_rep();
        self.streams.insert(rep, stream);
        Ok(Resource::new_own(rep))
    }
}

fn source_plugin_error_internal(error: impl std::fmt::Display) -> source_sidecar::PluginError {
    source_sidecar::PluginError::Internal(error.to_string())
}

fn source_transport_option_from(option: source_sidecar::TransportOption) -> SidecarTransportOption {
    SidecarTransportOption {
        kind: match option.kind {
            source_sidecar::TransportKind::Stdio => SidecarTransportKind::Stdio,
            source_sidecar::TransportKind::NamedPipe => SidecarTransportKind::NamedPipe,
            source_sidecar::TransportKind::UnixSocket => SidecarTransportKind::UnixSocket,
            source_sidecar::TransportKind::LoopbackTcp => SidecarTransportKind::LoopbackTcp,
            source_sidecar::TransportKind::SharedMemoryRing => {
                SidecarTransportKind::SharedMemoryRing
            },
        },
        priority: option.priority,
        max_frame_bytes: option.max_frame_bytes,
    }
}

fn source_transport_kind_into(kind: SidecarTransportKind) -> source_sidecar::TransportKind {
    match kind {
        SidecarTransportKind::Stdio => source_sidecar::TransportKind::Stdio,
        SidecarTransportKind::NamedPipe => source_sidecar::TransportKind::NamedPipe,
        SidecarTransportKind::UnixSocket => source_sidecar::TransportKind::UnixSocket,
        SidecarTransportKind::LoopbackTcp => source_sidecar::TransportKind::LoopbackTcp,
        SidecarTransportKind::SharedMemoryRing => source_sidecar::TransportKind::SharedMemoryRing,
    }
}

impl source_sidecar::Host for SourceStoreData {
    fn launch(
        &mut self,
        spec: source_sidecar::LaunchSpec,
    ) -> std::result::Result<Resource<source_sidecar::Process>, source_sidecar::PluginError> {
        let process_rep = self
            .sidecar
            .launch(&SidecarLaunchSpec {
                executable: resolve_sidecar_executable(&self.plugin_root, &spec.executable)
                    .map_err(source_plugin_error_internal)?,
                args: spec.args,
                preferred_control: spec
                    .preferred_control
                    .into_iter()
                    .map(source_transport_option_from)
                    .collect::<Vec<_>>(),
                preferred_data: spec
                    .preferred_data
                    .into_iter()
                    .map(source_transport_option_from)
                    .collect::<Vec<_>>(),
                env: spec.env,
            })
            .map_err(source_plugin_error_internal)?;
        Ok(Resource::new_own(process_rep))
    }
}

impl source_sidecar::HostProcess for SourceStoreData {
    fn open_control(
        &mut self,
        self_: Resource<source_sidecar::Process>,
    ) -> std::result::Result<Resource<source_sidecar::Channel>, source_sidecar::PluginError> {
        let process_rep = self_.rep();
        let channel_rep = self
            .sidecar
            .open_control(process_rep)
            .map_err(source_plugin_error_internal)?;
        Ok(Resource::new_own(channel_rep))
    }

    fn open_data(
        &mut self,
        self_: Resource<source_sidecar::Process>,
        role: String,
        preferred: Vec<source_sidecar::TransportOption>,
    ) -> std::result::Result<Resource<source_sidecar::Channel>, source_sidecar::PluginError> {
        let process_rep = self_.rep();
        let preferred = preferred
            .into_iter()
            .map(source_transport_option_from)
            .collect::<Vec<_>>();
        let channel_rep = self
            .sidecar
            .open_data(process_rep, role.trim(), &preferred)
            .map_err(source_plugin_error_internal)?;
        Ok(Resource::new_own(channel_rep))
    }

    fn wait_exit(
        &mut self,
        self_: Resource<source_sidecar::Process>,
        timeout_ms: Option<u32>,
    ) -> std::result::Result<Option<i32>, source_sidecar::PluginError> {
        let process_rep = self_.rep();
        self.sidecar
            .wait_exit(process_rep, timeout_ms)
            .map_err(source_plugin_error_internal)
    }

    fn terminate(
        &mut self,
        self_: Resource<source_sidecar::Process>,
        grace_ms: u32,
    ) -> std::result::Result<(), source_sidecar::PluginError> {
        let process_rep = self_.rep();
        self.sidecar
            .terminate(process_rep, grace_ms)
            .map_err(source_plugin_error_internal)
    }

    fn drop(&mut self, rep: Resource<source_sidecar::Process>) -> wasmtime::Result<()> {
        self.sidecar.drop_process(rep.rep());
        Ok(())
    }
}

impl source_sidecar::HostChannel for SourceStoreData {
    fn transport(
        &mut self,
        self_: Resource<source_sidecar::Channel>,
    ) -> source_sidecar::TransportKind {
        let channel_rep = self_.rep();
        self.sidecar
            .channel_transport(channel_rep)
            .map(source_transport_kind_into)
            .unwrap_or(source_sidecar::TransportKind::Stdio)
    }

    fn write(
        &mut self,
        self_: Resource<source_sidecar::Channel>,
        data: Vec<u8>,
    ) -> std::result::Result<u32, source_sidecar::PluginError> {
        let channel_rep = self_.rep();
        self.sidecar
            .channel_write(channel_rep, &data)
            .map_err(source_plugin_error_internal)
    }

    fn read(
        &mut self,
        self_: Resource<source_sidecar::Channel>,
        max_bytes: u32,
        timeout_ms: Option<u32>,
    ) -> std::result::Result<Vec<u8>, source_sidecar::PluginError> {
        let channel_rep = self_.rep();
        self.sidecar
            .channel_read(channel_rep, max_bytes, timeout_ms)
            .map_err(source_plugin_error_internal)
    }

    fn close(&mut self, self_: Resource<source_sidecar::Channel>) {
        let _ = self.sidecar.channel_close(self_.rep());
    }

    fn drop(&mut self, rep: Resource<source_sidecar::Channel>) -> wasmtime::Result<()> {
        self.sidecar.drop_channel(rep.rep());
        Ok(())
    }
}
