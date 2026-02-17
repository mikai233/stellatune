# `stellatune-audio` Rustdoc Execution Plan

Status: Active  
Last Updated: 2026-02-17  
Owner: `crates/stellatune-audio`

## Scope

This document tracks the end-to-end documentation work for `crates/stellatune-audio`.
The target style is close to Rust standard library documentation: factual, concise, and behavior-first.
The execution is split into two tracks:

1. Public API documentation (completed).
2. Internal implementation documentation (completed).

## Goals

1. Provide clear crate-level and module-level entry points for first-time readers.
2. Document all public APIs with stable semantics, including error behavior.
3. Standardize docs structure (`Errors`, `Examples`, terminology, linking style).
4. Keep docs verifiable with rustdoc/doctest checks.
5. Add internal implementation docs for critical private modules/functions and verify private-item doc builds.

## Deliverables

1. Documentation style guide for this crate.
2. Public API inventory and documentation gap report.
3. Crate-level docs in `src/lib.rs`.
4. Module-level docs for `config`, `engine`, `error`, and `pipeline`.
5. Public item docs for engine handle methods and core public types.
6. Working examples and rustdoc validation commands.
7. Internal module/function doc coverage baseline and progress tracking.

## Work Board

- [x] D1. Create execution plan and tracking document in `docs/`.
- [x] D2. Define documentation style guide for `stellatune-audio`.
- [x] D3. Build initial public API inventory and gap baseline.
- [x] D4. Add first crate-level rustdoc pass (`lib.rs`) and engine entry docs.
- [x] D5. Add module-level rustdoc for public modules (`config`, `engine`, `error`, `pipeline`).
- [x] D6. Add public item rustdoc for high-traffic APIs (`EngineHandle` methods first).
- [x] D7. Add/normalize examples (`no_run` by default where runtime setup is required).
- [x] D8. Run final rustdoc quality pass and close remaining gaps.
- [x] D9. Build internal-item inventory (private modules, private impl hot paths).
- [x] D10. Add module-level docs for private architecture modules (`infra`, `workers`, `pipeline::runtime`).
- [x] D11. Add docs for critical private functions in decode/recovery/sink lifecycle paths.
- [x] D12. Validate private-item doc generation (`cargo doc --document-private-items`) and record coverage status.

## Progress Log

### 2026-02-17

Completed:
- Public API documentation track (`D1-D8`) completed.
- Created this plan and tracking document.
- Added style rules: `docs/style/stellatune-audio-doc-style.md`.
- Collected initial API baseline from:
  - `crates/stellatune-audio/src/lib.rs`
  - `crates/stellatune-audio/src/engine/mod.rs`
  - `crates/stellatune-audio/src/engine/handle/*.rs`
- Started implementation:
  - Added crate-level docs in `crates/stellatune-audio/src/lib.rs`.
  - Added docs for engine entry points in `crates/stellatune-audio/src/engine/mod.rs`.
  - Added module-level docs in:
    - `crates/stellatune-audio/src/config/mod.rs`
    - `crates/stellatune-audio/src/pipeline/mod.rs`
    - `crates/stellatune-audio/src/error.rs`
    - `crates/stellatune-audio/src/engine/mod.rs`
  - Added high-traffic API docs for:
    - `crates/stellatune-audio/src/engine/handle/mod.rs`
    - `crates/stellatune-audio/src/engine/handle/transport.rs`
    - `crates/stellatune-audio/src/engine/handle/control_ops.rs`
    - `crates/stellatune-audio/src/engine/handle/pipeline_ops.rs`
  - Added `no_run` examples for key engine entry points and handle operations.
  - Added docs for core configuration and event types:
    - `crates/stellatune-audio/src/config/engine.rs`
    - `crates/stellatune-audio/src/config/gain.rs`
    - `crates/stellatune-audio/src/config/sink.rs`
  - Added docs for pipeline public API surface:
    - `crates/stellatune-audio/src/pipeline/assembly.rs`
    - `crates/stellatune-audio/src/pipeline/graph.rs`
  - Validation: `cargo check -p stellatune-audio` passed.
  - Validation: `cargo test -p stellatune-audio --doc` passed.
  - Validation: `RUSTDOCFLAGS="-D warnings" cargo doc -p stellatune-audio --no-deps` passed.
  - Validation: `cargo rustdoc -p stellatune-audio -- -W missing-docs` passed with no warnings.

Completed:
- Internal implementation documentation track (`D9-D12`) completed.
- Added private/module docs for internal architecture modules:
  - `crates/stellatune-audio/src/infra/mod.rs`
  - `crates/stellatune-audio/src/workers/mod.rs`
  - `crates/stellatune-audio/src/workers/decode/mod.rs`
  - `crates/stellatune-audio/src/workers/sink/mod.rs`
  - `crates/stellatune-audio/src/pipeline/runtime/mod.rs`
- Added private function docs for critical runtime paths:
  - Decode loop and recovery:
    - `crates/stellatune-audio/src/workers/decode/loop.rs`
    - `crates/stellatune-audio/src/workers/decode/recovery.rs`
    - `crates/stellatune-audio/src/workers/decode/util.rs`
  - Runner internals:
    - `crates/stellatune-audio/src/pipeline/runtime/runner/mod.rs`
    - `crates/stellatune-audio/src/pipeline/runtime/runner/lifecycle.rs`
    - `crates/stellatune-audio/src/pipeline/runtime/runner/step.rs`
    - `crates/stellatune-audio/src/pipeline/runtime/runner/control.rs`
  - Sink worker lifecycle:
    - `crates/stellatune-audio/src/workers/sink/worker.rs`
- Validation:
  - `cargo check -p stellatune-audio` passed.
  - `cargo doc -p stellatune-audio --no-deps --document-private-items` passed.

## Current Baseline Notes

1. Public API surface is concentrated in:
   - `lib.rs` module exports
   - `engine::start_engine*`
   - `engine::EngineHandle` async operations
2. Error model is now typed (`EngineError`, `DecodeError`) and should be referenced consistently in docs.
3. Some outer layers still convert to string-based errors; this should be called out where relevant in boundary docs.
4. `missing_docs` lint was used to validate public item coverage. It does not enforce private-item documentation.
5. Private module/function docs were added for the critical runtime paths tracked by `D9-D12`.

## Definition of Done

1. Public API track:
   - All public items in `crates/stellatune-audio` have rustdoc comments.
   - Public operations that can fail have an `# Errors` section.
   - Core user-facing APIs have examples.
2. Internal implementation track:
   - Private architecture modules have module-level docs.
   - Critical private functions in decode/recovery/sink paths are documented.
   - Private-item documentation builds successfully.
3. Validation commands:
   - `cargo check -p stellatune-audio`
   - `cargo test -p stellatune-audio --doc`
   - `RUSTDOCFLAGS="-D warnings" cargo doc -p stellatune-audio --no-deps`
   - `cargo doc -p stellatune-audio --no-deps --document-private-items`
