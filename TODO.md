# StellaTune Project TODO

## Plugin Platform Focus (Current Priority)

### 1) Improve plugin error handling and UI visibility
- Define a structured plugin error model (error code, plugin id, capability, action, detail, severity).
- Return structured load/reload/operation errors over FFI instead of only log strings.
- Emit dedicated runtime events for plugin errors in player and library pipelines.
- Show plugin errors in UI status panels (Settings, Sources, playback context) with actionable messages.
- Keep a recent plugin error timeline so users can diagnose failures after restart.

### 2) Add a JSON-based plugin UI panel system
- Define a JSON schema for host-rendered plugin panels (fields, groups, toggles, select, validation rules).
- Let plugins provide panel schema + default values for source, output sink, and lyrics capabilities.
- Let host UI render forms dynamically and send user input back as JSON payloads.
- Support form state persistence per plugin/type and schema version.
- Add runtime validation feedback (field-level errors and submit-level errors).

### 3) Remove `plugin.toml`, move to direct DLL metadata discovery
- Change plugin installation format to standalone DLL-based discovery.
- Read plugin metadata directly through exported plugin interface calls.
- Replace manifest-based discovery with binary scanning + metadata handshake.
- Keep install flow simple: copy DLL -> reload plugins -> show metadata and status.
- Add safety checks for duplicate plugin ids, invalid metadata, and ABI mismatch reporting.

## Capability Completion Roadmap

### A) Source plugins (custom input)
- Keep improving source catalog contracts for stable list/search/paging behavior.
- Add optional source auth/session hooks (token refresh, login state, permission failures).
- Add better source metadata normalization (title/artist/album/duration/cover).
- Make source plugin diagnostics visible in Sources page.

### B) Output sink plugins (custom output)
- Finalize single-output behavior and route transitions during active playback.
- Add output sink health events (open/write/flush failures, reconnect attempts).
- Add output sink capability reporting (accepted sample rates/channels/latency hints).
- Add fallback policy configuration when plugin output fails.

### C) Lyrics plugins
- Integrate plugin lyrics providers into the active lyrics pipeline (not only built-in online providers).
- Add provider selection policy (auto/manual, priority order, source filtering).
- Cache and conflict resolution rules for multiple providers.

## Technical Hardening
- Standardize plugin API error code mapping across host, FFI, and Dart UI.
- Expand tests for plugin load/reload, source open_stream decode path, and output sink write loop.
- Add regression tests for ABI mismatch, partial load failure, and poisoned lock recovery.
- Improve logging correlation (request id / track id / plugin id) for end-to-end debugging.

