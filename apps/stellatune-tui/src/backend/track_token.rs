use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
struct TrackRefToken<'a> {
    source_id: &'a str,
    track_id: &'a str,
    locator: &'a str,
}

pub fn encode_local_track_token(path: &str) -> String {
    let payload = TrackRefToken {
        source_id: "local",
        track_id: path,
        locator: path,
    };
    serde_json::to_string(&payload).unwrap_or_else(|_| path.to_string())
}

pub fn decode_track_token_path(track_token: &str) -> String {
    serde_json::from_str::<serde_json::Value>(track_token)
        .ok()
        .and_then(|value| {
            value
                .get("locator")
                .and_then(serde_json::Value::as_str)
                .map(ToString::to_string)
        })
        .unwrap_or_else(|| track_token.to_string())
}
