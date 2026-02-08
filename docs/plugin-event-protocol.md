# Plugin Event Protocol (v1 In-Place)

This document describes the runtime event/control JSON contract currently wired in StellaTune.

## Envelope

Plugin runtime events are moved as JSON strings.

- `kind`: one of `notify`, `control`, `control_result`, `control_finished`
- `plugin_id`: plugin id set by host
- `payload_json`: JSON string payload

In Rust/Flutter, this is exposed as `PluginRuntimeEvent` with typed `kind` (`PluginRuntimeKind`).

Flutter can subscribe via:
- `plugin_runtime_events(player, ...)` (legacy shape)
- `plugin_runtime_events_global(...)` (no `Player` handle required)

Plugin SDK (`stellatune-plugin-sdk`) now also provides typed helpers:
- build/send control requests:
  - typed builders (recommended):
    - `PlayerControl::seek_ms(...).request().request_id_str(...).send()`
    - `LibraryControl::list_tracks(...).request().request_id_str(...).send()`
- parse host-poll events:
  - `parse_host_event_json(...)`
  - `host_poll_event(...)`
  - `control_event_matches_request_id(...)`
- request id helper:
  - `next_request_id()`
- output sink implementation helper:
  - `OutputSink` / `OutputSinkDescriptor`
  - `SourceStream` / `SourceCatalogDescriptor`
  - `export_output_sinks_interface! { sinks: [...] }`
  - `export_source_catalogs_interface! { sources: [...] }`
  - `compose_get_interface! { fn ...; ... }`

Typed-builder example (recommended):

```rust
use stellatune_plugin_sdk::{LibraryControl, LibraryListTracksQuery, PlayerControl};

let ack = PlayerControl::seek_ms(30_000)
    .request()
    .request_id_str("req-typed-1")
    .send()?;
if !ack.ok {
    // optional: inspect ack.error
}

let query = LibraryListTracksQuery::new()
    .folder("")
    .recursive(true)
    .query("radiohead")
    .limit(100)
    .offset(0);
let _ = LibraryControl::list_tracks(query)
    .request()
    .request_id_str("req-typed-2")
    .send()?;
```

## Directions

1. Host/Flutter -> Plugin
- API: `plugin_publish_event_json(plugin_id, event_json)`
- If `plugin_id` is `null`, the event is broadcast to all loaded plugins.
- Plugin polls with SDK `host_poll_event()`.

2. Plugin -> Host/Flutter
- SDK: `host_emit_event_json(event_json)`
- Host pushes it into runtime queue and forwards as `PluginRuntimeEvent` stream.

3. Plugin -> Host control
- SDK: typed control builders (`PlayerControl` / `LibraryControl`)
- Current host behavior:
  - pushes event into runtime queue with `kind=control`
  - returns immediate `{"ok":true}` (accepted)
  - real execution happens in async router thread
  - host sends control result event back to the same plugin queue
  - Flutter runtime stream also receives `kind=control_result` with same payload JSON
  - host additionally sends control finished event when command is observed complete (or timeout/error)
  - Flutter runtime stream also receives `kind=control_finished`

## Control Payload

`payload_json` must be a JSON object.

Common fields:
- `scope`: `player` or `library` (default `player`)
- `command`: command string
- `request_id`: optional string; echoed in control result

### Player Scope Commands

- `load_track` with `path: string`
- `load_track_ref` with `track: TrackRef`
- `play`
- `pause`
- `stop`
- `shutdown`
- `refresh_devices`
- `seek_ms` with `position_ms: u64`
- `set_volume` with `volume: f64` (0..1 expected)
- `set_lfe_mode` with `mode: "Mute" | "MixToFront"` (also accepts `mute`/`mix_to_front`)
- `set_output_device` with:
  - `backend: "Shared" | "WasapiExclusive"` (also accepts `shared`/`wasapi_exclusive`)
  - `device_id: string | null` (optional)
- `set_output_options` with:
  - `match_track_sample_rate: bool`
  - `gapless_playback: bool`
  - `seek_track_fade: bool`
- `set_output_sink_route` with `route: OutputSinkRoute`
- `clear_output_sink_route`
- `preload_track` with:
  - `path: string`
  - optional `position_ms: u64` (default 0)
- `preload_track_ref` with:
  - `track: TrackRef`
  - optional `position_ms: u64` (default 0)

Example:

```json
{
  "scope": "player",
  "command": "seek_ms",
  "request_id": "req-1",
  "position_ms": 12345
}
```

### Library Scope Commands

- `add_root` with `path: string`
- `remove_root` with `path: string`
- `delete_folder` with `path: string`
- `restore_folder` with `path: string`
- `scan_all`
- `scan_all_force`
- `list_roots`
- `list_folders`
- `list_excluded_folders`
- `list_tracks` with optional:
  - `folder: string` (default `""`)
  - `recursive: bool` (default `true`)
  - `query: string` (default `""`)
  - `limit: i64` (default `5000`)
  - `offset: i64` (default `0`)
- `search` with optional:
  - `query: string` (default `""`)
  - `limit: i64` (default `200`)
  - `offset: i64` (default `0`)
- `list_playlists`
- `create_playlist` with `name: string`
- `rename_playlist` with `id: i64`, `name: string`
- `delete_playlist` with `id: i64`
- `list_playlist_tracks` with:
  - `playlist_id: i64`
  - optional `query`, `limit`, `offset`
- `add_track_to_playlist` with `playlist_id: i64`, `track_id: i64`
- `add_tracks_to_playlist` with `playlist_id: i64`, `track_ids: i64[]`
- `remove_track_from_playlist` with `playlist_id: i64`, `track_id: i64`
- `remove_tracks_from_playlist` with `playlist_id: i64`, `track_ids: i64[]`
- `move_track_in_playlist` with `playlist_id: i64`, `track_id: i64`, `new_index: i64`
- `list_liked_track_ids`
- `set_track_liked` with `track_id: i64`, `liked: bool`
- `shutdown`

Example:

```json
{
  "scope": "library",
  "command": "list_tracks",
  "request_id": "req-2",
  "folder": "",
  "recursive": true,
  "query": "radiohead",
  "limit": 100,
  "offset": 0
}
```

## Control Result Event

Host sends this JSON to plugin host-event queue after routing a control command:

```json
{
  "topic": "host.control.result",
  "request_id": "req-1",
  "scope": "player",
  "command": "seek_ms",
  "ok": true
}
```

## Control Finished Event

`host.control.finished` means command execution reached a terminal state in host routing layer.

Success example:

```json
{
  "topic": "host.control.finished",
  "request_id": "req-1",
  "scope": "player",
  "command": "play",
  "ok": true
}
```

Timeout example:

```json
{
  "topic": "host.control.finished",
  "request_id": "req-1",
  "scope": "library",
  "command": "scan_all",
  "ok": false,
  "error": "control finish timeout"
}
```

Failed case adds:

```json
{
  "topic": "host.control.result",
  "request_id": "req-1",
  "scope": "player",
  "command": "seek_ms",
  "ok": false,
  "error": "missing `position_ms`"
}
```

## Host Tick Event

Host currently broadcasts `player.tick` to plugins:

```json
{
  "topic": "player.tick",
  "state": "Playing",
  "position_ms": 1234,
  "track": "C:/music/a.flac",
  "wants_playback": true
}
```

## Host Runtime Event Broadcast

Host also broadcasts framework events to all plugins:

- `topic = "player.event"` with field `event` containing serialized `Event`
- `topic = "library.event"` with field `event` containing serialized `LibraryEvent`
