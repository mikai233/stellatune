mod decoder;
mod source;

use decoder::StreamDecoder;
use source::NeteaseSourceCatalog;
use stellatune_plugin_sdk::{compose_get_interface, export_source_catalogs_interface};

export_source_catalogs_interface! {
    sources: [
        netease_source => NeteaseSourceCatalog,
    ],
}

compose_get_interface! {
    fn __st_get_interface;
    __st_source_catalogs_get_interface,
}

stellatune_plugin_sdk::export_plugin! {
    id: "dev.stellatune.source.netease",
    name: "Netease Source (Sidecar)",
    version: (0, 1, 0),
    decoders: [
        stream => StreamDecoder,
    ],
    dsps: [],
    get_interface: __st_get_interface,
}
