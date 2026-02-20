use std::collections::BTreeMap;
use std::sync::Arc;

use wasmtime::component::Resource;

use stellatune_wasm_host_bindings::generated::decoder_plugin::stellatune::plugin::common as decoder_common;
use stellatune_wasm_host_bindings::generated::decoder_plugin::stellatune::plugin::host_stream as decoder_host_stream;
use stellatune_wasm_host_bindings::generated::decoder_plugin::stellatune::plugin::sidecar as decoder_sidecar;

use crate::executor::sidecar_state::SidecarState;
use crate::host::sidecar::{SidecarLaunchSpec, SidecarTransportKind, SidecarTransportOption};
use crate::host::stream::{HostStreamHandle, HostStreamService, StreamSeekWhence};

pub(crate) struct DecoderStoreData {
    pub(crate) stream_service: Arc<dyn HostStreamService>,
    pub(crate) next_rep: u32,
    pub(crate) streams: BTreeMap<u32, Box<dyn HostStreamHandle>>,
    pub(crate) sidecar: SidecarState,
}

impl DecoderStoreData {
    pub(crate) fn alloc_rep(&mut self) -> u32 {
        let rep = self.next_rep;
        self.next_rep = self.next_rep.saturating_add(1);
        if self.next_rep == 0 {
            self.next_rep = 1;
        }
        rep
    }
}

impl decoder_common::Host for DecoderStoreData {}

impl decoder_host_stream::HostHostStreamHandle for DecoderStoreData {
    fn read(
        &mut self,
        self_: Resource<decoder_host_stream::HostStreamHandle>,
        max_bytes: u32,
    ) -> std::result::Result<Vec<u8>, decoder_host_stream::PluginError> {
        let Some(stream) = self.streams.get_mut(&self_.rep()) else {
            return Err(decoder_host_stream::PluginError::NotFound(format!(
                "host stream handle `{}` not found",
                self_.rep()
            )));
        };
        stream
            .read(max_bytes)
            .map_err(|error| decoder_host_stream::PluginError::Internal(error.to_string()))
    }

    fn seek(
        &mut self,
        self_: Resource<decoder_host_stream::HostStreamHandle>,
        offset: i64,
        whence: decoder_host_stream::SeekWhence,
    ) -> std::result::Result<u64, decoder_host_stream::PluginError> {
        let Some(stream) = self.streams.get_mut(&self_.rep()) else {
            return Err(decoder_host_stream::PluginError::NotFound(format!(
                "host stream handle `{}` not found",
                self_.rep()
            )));
        };
        let whence = match whence {
            decoder_host_stream::SeekWhence::Start => StreamSeekWhence::Start,
            decoder_host_stream::SeekWhence::Current => StreamSeekWhence::Current,
            decoder_host_stream::SeekWhence::End => StreamSeekWhence::End,
        };
        stream
            .seek(offset, whence)
            .map_err(|error| decoder_host_stream::PluginError::Internal(error.to_string()))
    }

    fn tell(
        &mut self,
        self_: Resource<decoder_host_stream::HostStreamHandle>,
    ) -> std::result::Result<u64, decoder_host_stream::PluginError> {
        let Some(stream) = self.streams.get_mut(&self_.rep()) else {
            return Err(decoder_host_stream::PluginError::NotFound(format!(
                "host stream handle `{}` not found",
                self_.rep()
            )));
        };
        stream
            .tell()
            .map_err(|error| decoder_host_stream::PluginError::Internal(error.to_string()))
    }

    fn size(
        &mut self,
        self_: Resource<decoder_host_stream::HostStreamHandle>,
    ) -> std::result::Result<u64, decoder_host_stream::PluginError> {
        let Some(stream) = self.streams.get_mut(&self_.rep()) else {
            return Err(decoder_host_stream::PluginError::NotFound(format!(
                "host stream handle `{}` not found",
                self_.rep()
            )));
        };
        stream
            .size()
            .map_err(|error| decoder_host_stream::PluginError::Internal(error.to_string()))
    }

    fn close(&mut self, self_: Resource<decoder_host_stream::HostStreamHandle>) {
        if let Some(mut stream) = self.streams.remove(&self_.rep()) {
            stream.close();
        }
    }

    fn drop(
        &mut self,
        rep: Resource<decoder_host_stream::HostStreamHandle>,
    ) -> wasmtime::Result<()> {
        self.streams.remove(&rep.rep());
        Ok(())
    }
}

impl decoder_host_stream::Host for DecoderStoreData {
    fn open_uri(
        &mut self,
        uri: String,
    ) -> std::result::Result<
        Resource<decoder_host_stream::HostStreamHandle>,
        decoder_host_stream::PluginError,
    > {
        let stream = self
            .stream_service
            .open_uri(&uri)
            .map_err(|error| decoder_host_stream::PluginError::Internal(error.to_string()))?;
        let rep = self.alloc_rep();
        self.streams.insert(rep, stream);
        Ok(Resource::new_own(rep))
    }
}

fn decoder_plugin_error_internal(error: impl std::fmt::Display) -> decoder_sidecar::PluginError {
    decoder_sidecar::PluginError::Internal(error.to_string())
}

fn decoder_plugin_error_not_found(message: impl Into<String>) -> decoder_sidecar::PluginError {
    decoder_sidecar::PluginError::NotFound(message.into())
}

fn decoder_transport_option_from(
    option: decoder_sidecar::TransportOption,
) -> SidecarTransportOption {
    SidecarTransportOption {
        kind: match option.kind {
            decoder_sidecar::TransportKind::Stdio => SidecarTransportKind::Stdio,
            decoder_sidecar::TransportKind::NamedPipe => SidecarTransportKind::NamedPipe,
            decoder_sidecar::TransportKind::UnixSocket => SidecarTransportKind::UnixSocket,
            decoder_sidecar::TransportKind::LoopbackTcp => SidecarTransportKind::LoopbackTcp,
            decoder_sidecar::TransportKind::SharedMemoryRing => {
                SidecarTransportKind::SharedMemoryRing
            },
        },
        priority: option.priority,
        max_frame_bytes: option.max_frame_bytes,
    }
}

fn decoder_transport_kind_into(kind: SidecarTransportKind) -> decoder_sidecar::TransportKind {
    match kind {
        SidecarTransportKind::Stdio => decoder_sidecar::TransportKind::Stdio,
        SidecarTransportKind::NamedPipe => decoder_sidecar::TransportKind::NamedPipe,
        SidecarTransportKind::UnixSocket => decoder_sidecar::TransportKind::UnixSocket,
        SidecarTransportKind::LoopbackTcp => decoder_sidecar::TransportKind::LoopbackTcp,
        SidecarTransportKind::SharedMemoryRing => decoder_sidecar::TransportKind::SharedMemoryRing,
    }
}

impl decoder_sidecar::Host for DecoderStoreData {
    fn launch(
        &mut self,
        spec: decoder_sidecar::LaunchSpec,
    ) -> std::result::Result<Resource<decoder_sidecar::Process>, decoder_sidecar::PluginError> {
        let process_rep = self
            .sidecar
            .launch(&SidecarLaunchSpec {
                executable: spec.executable,
                args: spec.args,
                preferred_control: spec
                    .preferred_control
                    .into_iter()
                    .map(decoder_transport_option_from)
                    .collect::<Vec<_>>(),
                preferred_data: spec
                    .preferred_data
                    .into_iter()
                    .map(decoder_transport_option_from)
                    .collect::<Vec<_>>(),
                env: spec.env,
            })
            .map_err(decoder_plugin_error_internal)?;
        Ok(Resource::new_own(process_rep))
    }
}

impl decoder_sidecar::HostProcess for DecoderStoreData {
    fn open_control(
        &mut self,
        self_: Resource<decoder_sidecar::Process>,
    ) -> std::result::Result<Resource<decoder_sidecar::Channel>, decoder_sidecar::PluginError> {
        let process_rep = self_.rep();
        let channel_rep = self
            .sidecar
            .open_control(process_rep)
            .map_err(|error| decoder_plugin_error_not_found(error.to_string()))?;
        Ok(Resource::new_own(channel_rep))
    }

    fn open_data(
        &mut self,
        self_: Resource<decoder_sidecar::Process>,
        role: String,
        preferred: Vec<decoder_sidecar::TransportOption>,
    ) -> std::result::Result<Resource<decoder_sidecar::Channel>, decoder_sidecar::PluginError> {
        let process_rep = self_.rep();
        let preferred = preferred
            .into_iter()
            .map(decoder_transport_option_from)
            .collect::<Vec<_>>();
        let channel_rep = self
            .sidecar
            .open_data(process_rep, role.trim(), &preferred)
            .map_err(decoder_plugin_error_internal)?;
        Ok(Resource::new_own(channel_rep))
    }

    fn wait_exit(
        &mut self,
        self_: Resource<decoder_sidecar::Process>,
        timeout_ms: Option<u32>,
    ) -> std::result::Result<Option<i32>, decoder_sidecar::PluginError> {
        let process_rep = self_.rep();
        self.sidecar
            .wait_exit(process_rep, timeout_ms)
            .map_err(decoder_plugin_error_internal)
    }

    fn terminate(
        &mut self,
        self_: Resource<decoder_sidecar::Process>,
        grace_ms: u32,
    ) -> std::result::Result<(), decoder_sidecar::PluginError> {
        let process_rep = self_.rep();
        self.sidecar
            .terminate(process_rep, grace_ms)
            .map_err(decoder_plugin_error_internal)
    }

    fn drop(&mut self, rep: Resource<decoder_sidecar::Process>) -> wasmtime::Result<()> {
        self.sidecar.drop_process(rep.rep());
        Ok(())
    }
}

impl decoder_sidecar::HostChannel for DecoderStoreData {
    fn transport(
        &mut self,
        self_: Resource<decoder_sidecar::Channel>,
    ) -> decoder_sidecar::TransportKind {
        let channel_rep = self_.rep();
        self.sidecar
            .channel_transport(channel_rep)
            .map(decoder_transport_kind_into)
            .unwrap_or(decoder_sidecar::TransportKind::Stdio)
    }

    fn write(
        &mut self,
        self_: Resource<decoder_sidecar::Channel>,
        data: Vec<u8>,
    ) -> std::result::Result<u32, decoder_sidecar::PluginError> {
        let channel_rep = self_.rep();
        self.sidecar
            .channel_write(channel_rep, &data)
            .map_err(decoder_plugin_error_internal)
    }

    fn read(
        &mut self,
        self_: Resource<decoder_sidecar::Channel>,
        max_bytes: u32,
        timeout_ms: Option<u32>,
    ) -> std::result::Result<Vec<u8>, decoder_sidecar::PluginError> {
        let channel_rep = self_.rep();
        self.sidecar
            .channel_read(channel_rep, max_bytes, timeout_ms)
            .map_err(decoder_plugin_error_internal)
    }

    fn close(&mut self, self_: Resource<decoder_sidecar::Channel>) {
        let _ = self.sidecar.channel_close(self_.rep());
    }

    fn drop(&mut self, rep: Resource<decoder_sidecar::Channel>) -> wasmtime::Result<()> {
        self.sidecar.drop_channel(rep.rep());
        Ok(())
    }
}
