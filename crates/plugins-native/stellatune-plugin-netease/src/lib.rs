mod decoder;
mod source;

use decoder::StreamDecoderInstance;
use source::NeteaseSourceCatalogInstance;

stellatune_plugin_sdk::export_plugin! {
    id: "dev.stellatune.source.netease",
    name: "Netease Source (Sidecar)",
    version: (0, 1, 0),
    decoders: [
        stream => StreamDecoderInstance,
    ],
    dsps: [],
    source_catalogs: [
        netease_source => NeteaseSourceCatalogInstance,
    ],
    lyrics_providers: [],
    output_sinks: [],
}
