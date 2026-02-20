pub mod generated;

pub const WIT_DIR: &str = "../../wit/stellatune-plugin";

pub const WORLD_DECODER_PLUGIN: &str = "decoder-plugin";
pub const WORLD_SOURCE_PLUGIN: &str = "source-plugin";
pub const WORLD_LYRICS_PLUGIN: &str = "lyrics-plugin";
pub const WORLD_OUTPUT_SINK_PLUGIN: &str = "output-sink-plugin";
pub const WORLD_DSP_PLUGIN: &str = "dsp-plugin";

#[allow(dead_code)]
fn _touch_all_world_modules_for_compile() {
    let _ = core::mem::size_of::<generated::decoder_plugin::DecoderPlugin>();
    let _ = core::mem::size_of::<generated::source_plugin::SourcePlugin>();
    let _ = core::mem::size_of::<generated::lyrics_plugin::LyricsPlugin>();
    let _ = core::mem::size_of::<generated::output_sink_plugin::OutputSinkPlugin>();
    let _ = core::mem::size_of::<generated::dsp_plugin::DspPlugin>();
}
