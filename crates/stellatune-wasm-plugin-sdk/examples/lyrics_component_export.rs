use stellatune_wasm_plugin_sdk::prelude::*;

pub struct ExampleLyricsPlugin;
pub struct ExampleProvider;

impl PluginLifecycle for ExampleLyricsPlugin {}

impl ConfigStateOps for ExampleProvider {}

impl LyricsProvider for ExampleProvider {
    fn search(&mut self, keyword: &str) -> SdkResult<Vec<LyricCandidate>> {
        if keyword.trim().is_empty() {
            return Ok(Vec::new());
        }
        Ok(vec![LyricCandidate {
            id: "demo-1".to_string(),
            title: format!("match: {keyword}"),
            artist: "sdk-example".to_string(),
        }])
    }

    fn fetch(&mut self, id: &str) -> SdkResult<String> {
        Ok(format!("[00:00.00] lyric payload for {id}"))
    }
}

impl LyricsPlugin for ExampleLyricsPlugin {
    type Provider = ExampleProvider;
    const TYPE_ID: &'static str = "lyrics-example";
    const DISPLAY_NAME: &'static str = "Lyrics Export Example";

    fn create_provider(&mut self) -> SdkResult<Self::Provider> {
        Ok(ExampleProvider)
    }
}

fn create_plugin() -> SdkResult<ExampleLyricsPlugin> {
    Ok(ExampleLyricsPlugin)
}

stellatune_wasm_plugin_sdk::export_lyrics_component! {
    plugin_type: crate::ExampleLyricsPlugin,
    create: crate::create_plugin,
}

fn main() {}
