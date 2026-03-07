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

### D) Encoder plugins (custom transcode/export)
- Flesh out standard presets for target formats (FLAC/MP3/AAC/WAV).
- Expose encoder progress events to handle long-running batch transcodes.
- Add tag rewriting layer to mapped output streams.


## Technical Hardening
- Standardize plugin API error code mapping across host, FFI, and Dart UI.
- Expand tests for plugin load/reload, source open_stream decode path, and output sink write loop.
- Add regression tests for ABI mismatch, partial load failure, and poisoned lock recovery.
- Improve logging correlation (request id / track id / plugin id) for end-to-end debugging.

