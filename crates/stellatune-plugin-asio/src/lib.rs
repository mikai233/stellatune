mod client;
mod config;
mod descriptor;
mod instance;
mod ring;
mod sink;

use instance::AsioOutputSinkInstance;
use stellatune_plugin_sdk::export_plugin;

export_plugin! {
    id: "dev.stellatune.output.asio",
    name: "ASIO Output Sink",
    version: (0, 1, 0),
    decoders: [],
    dsps: [],
    source_catalogs: [],
    lyrics_providers: [],
    output_sinks: [
        asio => AsioOutputSinkInstance,
    ],
}
