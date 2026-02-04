mod decoder;
mod flac_offset;
mod io;
mod tags;

pub use decoder::NcmDecoder;

stellatune_plugin_sdk::export_plugin! {
  id: "dev.stellatune.decoder.ncm",
  name: "NCM Decoder (ncmdump)",
  version: (0, 1, 0),
  decoders: [
    ncm => NcmDecoder,
  ],
  dsps: [],
}
