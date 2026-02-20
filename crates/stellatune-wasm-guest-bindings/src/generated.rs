// Guest-side typed bindings generated from the canonical Stellatune WIT package.

pub mod decoder_plugin {
    wit_bindgen::generate!({
        path: "../../wit/stellatune-plugin",
        world: "decoder-plugin",
    });
}

pub mod source_plugin {
    wit_bindgen::generate!({
        path: "../../wit/stellatune-plugin",
        world: "source-plugin",
    });
}

pub mod dsp_plugin {
    wit_bindgen::generate!({
        path: "../../wit/stellatune-plugin",
        world: "dsp-plugin",
    });
}

pub mod lyrics_plugin {
    wit_bindgen::generate!({
        path: "../../wit/stellatune-plugin",
        world: "lyrics-plugin",
    });
}

pub mod output_sink_plugin {
    wit_bindgen::generate!({
        path: "../../wit/stellatune-plugin",
        world: "output-sink-plugin",
    });
}
