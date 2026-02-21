# Wasm Plugin Hard Cutover Execution Plan

Date: 2026-02-20

This plan assumes a direct replacement with no compatibility with the legacy dll plugin system.

## Phase 1 (Done): Package and Runtime Entry Cut

Goal:
- Stop using legacy package install/list/uninstall APIs.
- Route backend shared runtime entry to wasm runtime service.

Changes:
- `crates/stellatune-backend-api/src/player.rs`
- `crates/stellatune-backend-api/src/runtime/mod.rs`
- `crates/stellatune-backend-api/src/runtime/wasm_runtime.rs`

Exit criteria:
- `cargo check -p stellatune-backend-api` passes.
- Plugin package APIs are served by `stellatune-wasm-plugins::package::*`.

## Phase 2: Decoder Runtime Hard Switch

Goal:
- Remove runtime usage of `stellatune_plugins::runtime::*` in decoder path.
- Decoder probing/selection/instance lifecycle uses wasm runtime + wasm executor only.

Work items:
- Refactor `crates/stellatune-backend-api/src/runtime/hybrid_decoder_stage.rs`:
  - replace `shared_runtime_service` capability discovery.
  - replace `PluginDecoderStage` old worker path with wasm decoder plugin instance path.
- Refactor `crates/audio-adapters/stellatune-audio-plugin-adapters/src/decoder_stage.rs`.
- Refactor `crates/audio-adapters/stellatune-audio-plugin-adapters/src/decoder_runtime.rs`.
- Refactor metadata probe path:
  - `crates/stellatune-library/src/worker/metadata.rs`.

Exit criteria:
- No `stellatune_plugins::runtime::*` in decoder runtime path.
- Track decode works through wasm decoder plugin end-to-end.

## Phase 3: Output Sink and DSP Runtime Hard Switch

Goal:
- Replace old output sink + DSP worker endpoints with wasm plugin instance APIs.

Work items:
- `crates/audio-adapters/stellatune-audio-plugin-adapters/src/output_sink_stage.rs`
- `crates/audio-adapters/stellatune-audio-plugin-adapters/src/output_sink_runtime.rs`
- `crates/audio-adapters/stellatune-audio-plugin-adapters/src/transform_stage.rs`
- `crates/audio-adapters/stellatune-audio-plugin-adapters/src/transform_runtime.rs`
- `crates/stellatune-backend-api/src/runtime/engine.rs` route setup points.

Exit criteria:
- Output sink route and DSP chain run with wasm runtime only.
- No old worker controller/endpoints in audio execution path.

## Phase 4: Library and Backend Service Hard Switch

Goal:
- Remove legacy runtime usage from library/backend orchestration.

Work items:
- `crates/stellatune-library/src/service.rs`
- `crates/stellatune-library/src/worker/mod.rs`
- `crates/stellatune-backend-api/src/runtime/mod.rs` (remove temporary facade compatibility fields if any).

Exit criteria:
- `shared_runtime_service` from legacy crate is no longer referenced by library/backend.

## Phase 5: Remove Legacy Crate Dependencies

Goal:
- Delete direct dependencies on `stellatune-plugins` from runtime-critical crates.

Work items:
- Clean `Cargo.toml` dependencies:
  - `crates/stellatune-backend-api/Cargo.toml`
  - `crates/audio-adapters/stellatune-audio-plugin-adapters/Cargo.toml`
  - `crates/stellatune-library/Cargo.toml`
- Remove dead code and types imported from legacy runtime.

Exit criteria:
- `rg -n "stellatune_plugins::runtime|shared_runtime_service" crates/stellatune-backend-api crates/audio-adapters crates/stellatune-library` returns no runtime-path hits.

## Phase 6: Cleanup and Validation

Goal:
- Ensure the full product flow works with wasm-only plugin system.

Validation:
- Build:
  - `cargo check -p stellatune-backend-api`
  - `cargo check -p stellatune-library`
  - `cargo check -p stellatune-audio-plugin-adapters`
- Plugin flow:
  - install/list/uninstall via backend API.
  - plugin enable/disable/apply-state.
- Playback flow:
  - decoder plugin decode
  - output sink plugin route
  - dsp plugin processing

Exit criteria:
- Playback and plugin management pass with wasm-only runtime path.
- No legacy plugin runtime code used in production path.
