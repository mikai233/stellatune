use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(
    tag = "command",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum PlayerCommand {
    Play,
    Pause,
    Stop,
    Seek { position_ms: u64 },
    Next,
    Previous,
    PlayTrack { track_id: String },
    AppendQueue { track_ids: Vec<String> },
    ReplaceQueue { track_ids: Vec<String> },
    RemoveQueueItems { item_ids: Vec<String> },
    SelectItem { item_id: String },
    SetQueueMode { repeat: Repeat, shuffle: bool },
    PlayProviderTrack { track: ProviderTrack },
    EnqueueProviderTrack { track: ProviderTrack },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderTrack {
    pub plugin_id: String,
    pub capability_id: String,
    pub provider_id: String,
    pub provider_key: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Repeat {
    Off,
    All,
    One,
}
