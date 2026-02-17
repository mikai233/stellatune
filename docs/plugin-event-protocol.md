# Plugin Runtime Communication Model (Current)

This document replaces the old host-event JSON bus notes.

## Status

The legacy plugin runtime event/control channel has been removed.

In the current codebase, the following FFI APIs are intentionally disabled and
return an error:

- `plugin_runtime_events_global(...)`
- `plugin_publish_event_json(...)`

## What Replaced the Old Event Bus

Plugin integration now uses capability-specific worker endpoints and direct API
calls instead of a shared control/event JSON stream.

### 1. Capability Discovery

Host enumerates capabilities via plugin runtime introspection:

- active plugin ids
- per-plugin capability descriptors (`kind`, `type_id`, schema/default JSON)
- decoder extension cache for fast playability checks

### 2. Capability Invocation Through Worker Endpoints

Host binds a typed worker endpoint and then drives an instance controller.
This pattern is used for:

- source catalog capabilities
- lyrics provider capabilities
- output sink capabilities
- decoder / DSP capabilities (runtime internal paths)

The worker controller model is:

1. bind endpoint (`bind_*_worker_endpoint`)
2. construct controller (`into_controller`)
3. apply pending lifecycle (`apply_pending`)
4. access/drive live instance

### 3. Runtime Control for Worker Instances

Worker reconfiguration no longer travels through global JSON control events.
It is handled by `WorkerControlMessage` with ordered sequencing:

- `Recreate { seq, ... }`
- `Destroy { seq, ... }`

Controllers ignore stale sequence values and apply only the latest control intent.

## Flutter/FFI Surface Today

For plugin-backed features, Flutter calls direct APIs rather than publishing to
a plugin event bus, for example:

- `source_list_items_json(...)`
- `lyrics_provider_search_json(...)`
- `lyrics_provider_fetch_json(...)`
- `output_sink_list_targets_json(...)`
- `set_output_sink_route(...)` / `clear_output_sink_route(...)`

For audio playback state, Flutter should consume `events(...)` from the engine
event stream, not plugin runtime event streams.

## Plugin SDK Surface Today

`stellatune-plugin-sdk` currently focuses on host context and runtime utilities:

- logging (`host_log`)
- runtime root path helpers
- sidecar launch helpers (`sidecar_command`, `spawn_sidecar`)

Legacy SDK helpers around host event polling/emitting are no longer part of the
current integration path.

## Migration Summary

Old model:

- broadcast/point-to-point JSON events over host runtime queue
- plugin control requests expressed as generic JSON command payloads

Current model:

- capability introspection + typed endpoint binding
- per-capability instance lifecycle controllers
- direct host API calls for player/library/runtime operations
