use crate::capabilities::{AbilityDescriptor, AbilityKind, ConfigStateOps};
use crate::common::{EncodedChunk, MediaMetadata};
use crate::error::{SdkError, SdkResult};
use crate::host_stream::HostStreamOpenRequest;
use crate::lifecycle::PluginLifecycle;

pub enum OpenedSourceStream<S> {
    Processed {
        stream: S,
        ext_hint: Option<String>,
        metadata: Option<MediaMetadata>,
    },
    PassthroughRequest {
        request: HostStreamOpenRequest,
        ext_hint: Option<String>,
        metadata: Option<MediaMetadata>,
    },
}

impl<S> OpenedSourceStream<S> {
    pub fn processed(stream: S) -> Self {
        Self::Processed {
            stream,
            ext_hint: None,
            metadata: None,
        }
    }
}

pub trait SourceStream: Send {
    fn metadata(&self) -> SdkResult<MediaMetadata>;
    fn read(&mut self, max_bytes: u32) -> SdkResult<EncodedChunk>;
    fn close(&mut self) -> SdkResult<()> {
        Ok(())
    }
}

pub trait SourceCatalog: ConfigStateOps + Send {
    type Stream: SourceStream;

    fn list_items_json(&mut self, request_json: &str) -> SdkResult<String>;
    fn open_stream_json(&mut self, track_json: &str) -> SdkResult<Self::Stream>;
    fn open_stream_opened_json(
        &mut self,
        track_json: &str,
    ) -> SdkResult<OpenedSourceStream<Self::Stream>> {
        self.open_stream_json(track_json)
            .map(OpenedSourceStream::processed)
    }

    fn open_uri(&mut self, _uri: &str) -> SdkResult<Self::Stream> {
        Err(SdkError::unsupported("open-uri is unsupported"))
    }

    fn open_uri_opened(&mut self, uri: &str) -> SdkResult<OpenedSourceStream<Self::Stream>> {
        self.open_uri(uri).map(OpenedSourceStream::processed)
    }

    fn close(&mut self) -> SdkResult<()> {
        Ok(())
    }
}

pub trait SourcePlugin: PluginLifecycle + Send + 'static {
    type Catalog: SourceCatalog;

    const TYPE_ID: &'static str;
    const DISPLAY_NAME: &'static str;
    const CONFIG_SCHEMA_JSON: &'static str = "{}";
    const DEFAULT_CONFIG_JSON: &'static str = "{}";

    fn descriptor() -> AbilityDescriptor {
        AbilityDescriptor {
            kind: AbilityKind::Source,
            type_id: Self::TYPE_ID,
            display_name: Self::DISPLAY_NAME,
            config_schema_json: Self::CONFIG_SCHEMA_JSON,
            default_config_json: Self::DEFAULT_CONFIG_JSON,
        }
    }

    fn create_catalog(&mut self) -> SdkResult<Self::Catalog>;
}
