# Wasm Plugin System Parallel Implementation Plan

## Goal

Implement the new Wasm plugin system to production quality **without replacing** the current native plugin runtime yet.  
After the new system reaches feature/quality gates, decide whether to replace, dual-run, or keep hybrid mode.

## Principles

- Keep existing plugin system as the active runtime path.
- Build Wasm runtime as a parallel subsystem (`stellatune-wasm-plugins`).
- Avoid breaking backend/library APIs during implementation.
- Drive replacement decision with measurable criteria, not assumptions.

## Current Status

- Wasm package format + manifest validation implemented.
- Wasm plugin install/list/uninstall implemented.
- Wasm runtime registry implemented (load/discover/capability indexing).
- Runtime service boundary added for future backend integration:
  - reload from state
  - detailed sync report
  - disabled/enabled plugin state
  - capability lookup
  - unload/shutdown
- Existing backend package APIs are still routed to native plugin system.

## Phase 1: Runtime Core (Component Execution)

1. Add Wasmtime-based executor in `stellatune-wasm-plugins`.
2. Load plugin components from `plugin.json` component specs.
3. Validate world compatibility per component.
4. Implement plugin lifecycle hooks:
   - `on-enable`
   - `on-disable`
5. Create runtime instance model:
   - plugin-level state
   - component-level instance(s)
   - per-capability dispatch table

Deliverable:
- Plugins can be enabled/disabled and instantiated from manifest components.

## Phase 2: Host Imports (System Tools)

1. Implement host imports from WIT:
   - `host-stream`
   - `http-client`
   - `sidecar`
2. Sidecar lifecycle management:
   - start on plugin enable (if requested)
   - stop on plugin disable/unload
   - crash/restart policy
3. Sidecar IPC modes (manifest-driven):
   - stdio
   - shared memory
   - named pipe/unix socket (platform-specific)

Deliverable:
- Wasm plugins can consume host services and manage sidecar-backed behavior safely.

## Phase 3: Capability Execution Paths

1. Implement `output-sink` capability end-to-end in Wasm runtime.
2. Implement `dsp` capability end-to-end.
3. Add hot-path core ABI integration for low-latency processing:
   - shared memory region
   - pointer/length contracts
   - fallback to component calls for control plane
4. Keep decoder/source/lyrics as control-path component calls initially.

Deliverable:
- At least sink + dsp are runnable through Wasm runtime in parallel with native runtime.

## Phase 4: State, Config, and Isolation

1. Implement config plan/apply/export/import hooks.
2. Add plugin instance isolation rules and resource limits.
3. Add deterministic teardown semantics for sidecar and runtime instances.

Deliverable:
- Wasm plugins support runtime reconfiguration and stable multithread execution.

## Phase 5: Integration and Observability

1. Add backend APIs for Wasm runtime status and diagnostics.
2. Emit structured events/metrics:
   - lifecycle timings
   - sidecar starts/failures
   - hot-path underrun/overrun
   - memory and instance counts
3. Add failure taxonomy and actionable error surfaces.

Deliverable:
- Wasm runtime is operable and debuggable in production-like environments.

## Phase 6: Test Matrix

1. Unit tests:
   - manifest validation
   - capability indexing
   - lifecycle transitions
2. Integration tests:
   - enable/disable with sidecar
   - sink/dsp data flow
   - hot-reload and shutdown safety
3. Stress tests:
   - rapid reload
   - sidecar crash loops
   - concurrent pipeline sessions
4. Compatibility tests vs native behavior:
   - output correctness
   - latency envelope
   - error behavior

Deliverable:
- Quantitative confidence report for replacement decision.

## Replacement Decision Gates

Only consider replacing native plugin runtime when all gates pass:

1. Feature parity for required capabilities.
2. Stability:
   - no critical crash/leak in stress test.
3. Performance:
   - within target CPU/memory/latency budget.
4. Operational readiness:
   - diagnostics and recovery procedures verified.
5. Migration readiness:
   - existing plugin catalog has migration path or fallback.

## Near-Term Execution Order

1. Implement executor + lifecycle (Phase 1).
2. Implement host imports + sidecar lifecycle (Phase 2).
3. Deliver sink/dsp in Wasm runtime (Phase 3).
4. Run targeted benchmarks and stress tests.
5. Re-evaluate replacement strategy from real metrics.
