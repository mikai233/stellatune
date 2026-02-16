mod decoder;
mod flac_offset;
mod io;
mod tags;

use decoder::NcmDecoderInstance;

stellatune_plugin_sdk::export_plugin! {
  id: "dev.stellatune.decoder.ncm",
  name: "NCM Decoder (ncmdump)",
  version: (0, 1, 0),
  decoders: [
    ncm => NcmDecoderInstance,
  ],
  dsps: [],
  source_catalogs: [],
  lyrics_providers: [],
  output_sinks: [],
}
