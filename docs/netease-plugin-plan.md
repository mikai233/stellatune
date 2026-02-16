# Netease Plugin In-Repo Execution Notes

This document tracks the temporary in-repo implementation plan for Netease plugin testing.

## Scope

- Add plugin crate in this repository for fast integration tests.
- Keep sidecar and plugin out of default app distribution.
- Stabilize interfaces before moving to a standalone repository.

## Implemented Structure

- `crates/plugins-native/stellatune-plugin-netease`
  - `SourceCatalog` type: `netease`
  - Decoder type: `stream_symphonia`
- `tools/stellatune-ncm-sidecar`
  - Local HTTP wrapper around `NeteaseCloudMusicApi`

## Runtime Behavior

- Plugin calls sidecar through `sidecar_base_url` (default `http://127.0.0.1:46321`).
- Plugin always ensures sidecar process is running when source APIs are used.
- Supported sidecar executable candidates under plugin runtime root:
  - `stellatune-ncm-sidecar.exe`
  - `bin/stellatune-ncm-sidecar.exe`

## Next Milestones

1. Wire plugin runtime auth events (`qr_ready`, `auth_status`) to existing runtime debug panel.
2. Add source browsing UI (search + playlist entry points) for manual playback tests.
3. Expand stream decoder metadata fidelity and seek behavior verification.
4. Add CI scripts for plugin packaging smoke checks. (Done on 2026-02-09)
