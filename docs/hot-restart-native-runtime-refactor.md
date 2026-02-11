# Native Runtime Singleton Refactor (Hot Restart Stability)

Status: Draft  
Last Updated: 2026-02-11  
Owner: `stellatune-ffi` / `stellatune-backend-api` / `stellatune-audio` / `stellatune-plugin-asio` / Flutter desktop host  
Scope: Breaking redesign, no backward compatibility required.

## 1. Background

Flutter Desktop `Hot Restart` rebuilds Dart isolate, but does not fully restart native process.
Current architecture creates a new Rust-side player runtime on each `create_player()`.
This causes native resources to accumulate when old Rust opaque objects are not dropped in time:

1. duplicated audio engine control threads
2. duplicated ASIO sidecar processes
3. stale plugin generations in draining state
4. unreleased shared-memory ring files and handles
5. unstable FFI channel behavior after repeated restarts (`Lost Connection`)

## 2. Problem Statement

Current behavior has two core structural issues:

1. `create_player()` is *constructor semantics* (new engine/runtime), not *attach semantics*.
2. cleanup relies on GC/finalizer/drop timing across isolate boundaries.

For Hot Restart scenarios, this is fundamentally unsafe.

## 3. Refactor Policy

This refactor is intentionally breaking:

1. no compatibility shim for old lifecycle contract
2. no dual runtime mode (`legacy per-player` + `singleton runtime`)
3. no reliance on implicit drop for critical resource cleanup

Target is a single lifecycle model for desktop runtime.

## 4. Target Architecture

### 4.1 Process-Scoped Runtime Host

Introduce a process-global runtime host:

1. `RuntimeHost` owns exactly one `EngineHandle`
2. plugin runtime/router are bound once to this host
3. host provides generic lifecycle guarantees; plugin-specific resource ownership stays in plugin implementation
4. instantiated via `OnceLock<Arc<RuntimeHost>>`

`create_player()` no longer starts engine threads. It only returns a lightweight session/client handle bound to the singleton host.

### 4.2 Player Handle = Client Session

Replace current semantic:

1. Old: `Player` owns `PlayerService` owns engine lifecycle
2. New: `Player` owns `client_id`, references shared `RuntimeHost`

All player APIs forward to host + client context.

### 4.3 Explicit Lifecycle APIs

Add explicit lifecycle control in FFI (names can be finalized during implementation):

1. `runtime_init()` (idempotent)
2. `player_create_client() -> PlayerClient`
3. `player_dispose_client(client)` (explicit detach)
4. `runtime_prepare_hot_restart()` (dev-only, optional)
5. `runtime_shutdown()` (process exit / test teardown)

Important: correctness must not depend on `runtime_prepare_hot_restart()` being called.
Singleton model itself must survive repeated Hot Restart without leaks.

### 4.4 Plugin-Owned Sidecar Under Generic Lifecycle

Keep host/plugin boundaries strict:

1. host/runtime only exposes generic lifecycle mechanisms (instance close/drop order, generation deactivate/unload, explicit runtime shutdown hooks)
2. ASIO plugin owns sidecar process lifecycle in plugin-local manager
3. sidecar manager deduplicates spawn/reuse/stop by plugin-local policy
4. deterministic stop is triggered by generic lifecycle events, not plugin-id branches in host

This preserves ABI intent: host is capability-generic; plugin owns implementation-specific resources.

### 4.5 Optional Architecture Variant: Extract `RuntimeHost` to a Neutral Crate

To keep backend runtime ownership reusable across Flutter/TUI/CLI adapters, `RuntimeHost` can be moved from
`stellatune-backend-api` into a neutral crate (for example: `stellatune-runtime-host`).

Notes:

1. this extraction is optional for initial stabilization and can be deferred until Phase B/C are stable
2. if extracted, `stellatune-backend-api` and future frontends should consume the same host crate
3. ownership rule remains the same: runtime lifecycle belongs to backend runtime layer, not adapter-specific layers

## 5. Breaking API Changes

## 5.1 Rust Backend / FFI

1. `create_player()` behavior changes from "new runtime" to "attach client"
2. add explicit player dispose API (required by host)
3. add runtime-level init/shutdown APIs
4. remove per-player `register_plugin_runtime_engine(...)` calls

## 5.2 Dart / Flutter Layer

1. `PlayerBridge.create()` becomes attach-only
2. app bootstrap does not trigger a new native runtime per restart
3. app shutdown path explicitly detaches active player client
4. optional dev hook calls `runtime_prepare_hot_restart()` before/after restart boundary when supported

## 5.3 Plugin Runtime Contract

1. generation cleanup must support stale-client eviction policy
2. draining generations must not block forever due to orphaned client sessions
3. uninstall/reload path can force-release orphaned runtime instances owned by detached/expired clients

### 5.4 Plugin ABI Cleanup Semantics

Define cleanup behavior at ABI boundary (generic, plugin-agnostic):

1. `reset`: lightweight disruptive reset for live route/track transition; keep instance reusable.
2. `close`: deterministic runtime cleanup boundary; must release runtime-owned external resources (process handles, shared memory mappings, sockets/files), while allowing future reopen.
3. `destroy`: final lifetime boundary; host should call `close` before `destroy`; plugin must still fully clean up even if `close` was skipped.

ASIO plugin alignment:

1. `close` drops opened sink and releases sidecar lease/ring mapping deterministically.
2. sidecar process is stopped and torn down when last lease is released.

## 6. Migration Plan

## Phase A: Runtime Host Introduction

1. add `RuntimeHost` with `OnceLock`
2. move engine startup from `PlayerService::new()` into host init
3. make `PlayerService`/`Player` reference host instead of owning engine
4. keep existing public FFI methods forwarding to host

Exit criteria:

1. one process -> one control thread
2. repeated `create_player()` does not create new engine threads

## Phase B: Client Sessionization

1. add `client_id` registry in runtime host
2. map streams/subscriptions by `client_id`
3. implement `player_dispose_client`
4. enforce stale client cleanup

Exit criteria:

1. repeated attach/detach leaves no leaked client session
2. plugin generation draining count converges to zero after detach

## Phase C: ASIO Sidecar Ownership Consolidation

Sub-phases:

1. `C1` Plugin-local sidecar manager foundation
2. `C2` Route ASIO sink paths through plugin-local lease/manager
3. `C3` Wire generic lifecycle hooks (host generic, no plugin special-case) to guarantee deterministic plugin cleanup triggers
4. `C4` Stress verification and leak trend regression gate

Detailed actions:

1. replace ad-hoc persistent sidecar spawn with plugin-local lease manager
2. keep sidecar process ownership entirely in `stellatune-plugin-asio`
3. ensure sidecar stop on last lease release and plugin instance close/drop path
4. host only enforces generic lifecycle ordering; no `plugin_id == asio` branches

Exit criteria:

1. process list keeps max one active ASIO host for one active route
2. repeated Hot Restart does not increase ASIO host count
3. no host code introduces plugin-specific sidecar logic

## Phase D: Contract Cleanup

1. remove legacy per-player lifecycle assumptions
2. delete obsolete drop-dependent cleanup paths
3. simplify logs/metrics to singleton-runtime model

Exit criteria:

1. no code path starts engine outside runtime host
2. no lifecycle-critical cleanup depends only on finalizer timing

## 7. Verification Matrix

## 7.1 Functional

1. playback works after 20 consecutive Hot Restart cycles
2. output route apply/clear still works
3. plugin reload/uninstall/install still works
4. ASIO playback start/stop/seek/switch still works

## 7.2 Resource Stability

1. ASIO host process count remains bounded (target: `<=1` for active ASIO route)
2. control thread count remains bounded (target: singleton)
3. no growth trend in `.asio/ring-*.shm` after playback stop + restart cycles
4. plugin uninstall no longer reports persistent access-denied due to stale holders

## 7.3 FFI Stability

1. no `Lost Connection` in 30-minute restart + playback stress run
2. event streams recover after restart without duplicate flood
3. client detach + reattach sequence does not panic

## 8. Observability Requirements

Add or keep structured logs:

1. runtime host init/reuse/shutdown
2. client attach/detach and active client count
3. sidecar spawn/reuse/stop with reason
4. plugin generation active/draining counts
5. explicit warning on stale client eviction

Add metrics counters:

1. `runtime_host_inits_total`
2. `player_clients_active`
3. `asio_sidecar_spawns_total`
4. `asio_sidecar_running`
5. `plugin_generations_draining`

## 9. Risks and Mitigations

1. Risk: singleton runtime hides state bugs between restarts  
Mitigation: add explicit runtime reset API for tests; add deterministic integration tests.

2. Risk: event bus behavior changes for multiple clients  
Mitigation: define client-scoped vs global stream semantics explicitly before migration.

3. Risk: ASIO sidecar manager introduces lock contention  
Mitigation: isolate sidecar manager state; avoid holding mutex across blocking IO.

4. Risk: aggressive stale-client cleanup breaks legitimate long tasks  
Mitigation: use lease/heartbeat and grace period; log cleanup reason.

## 10. Acceptance Criteria (Release Gate)

All must pass:

1. 50 Hot Restart cycles without increasing ASIO host count trend
2. 0 `Lost Connection` in 30-minute stress scenario
3. plugin install/uninstall/reload paths stable under restart stress
4. no known resource leak trend in ring files/handles/processes
5. architecture invariant documented and enforced:
   1. one process -> one runtime host
   2. one runtime host -> deterministic lifecycle APIs

## 11. Implementation Checklist

- [x] Add `RuntimeHost` singleton and move engine init there
- [x] Convert `create_player()` to attach-client semantics
- [x] Add explicit `player_dispose_client` API in FFI + Dart bridge
- [x] Introduce client registry + stale-client eviction
- [x] Introduce plugin-local ASIO sidecar lease manager
- [x] Remove old ad-hoc per-instance sidecar ownership assumptions
- [ ] Add runtime/sidecar/generation metrics and logs
- [ ] Add restart stress integration test plan and scripts
- [x] Delete obsolete lifecycle code paths

## 12. Phase A Progress

- [x] A1. Introduce process-scoped `RuntimeHost` singleton (`OnceLock<Arc<...>>`) in backend runtime layer (2026-02-11)
- [x] A2. Move `start_engine()` + `register_plugin_runtime_engine(...)` into singleton init path (2026-02-11)
- [x] A3. Refactor `PlayerService` from owning `EngineHandle` to referencing shared `RuntimeHost` (2026-02-11)
- [x] A4. Build verification: `cargo check` passes for workspace after refactor (2026-02-11)
- [ ] A5. Hot Restart stress verification (create-player count no longer implies engine-thread growth)

## 13. Phase B Progress

- [x] B1. Add runtime client attach registry in `RuntimeHost` (`client_id` allocation + active count tracking) (2026-02-11)
- [x] B2. `PlayerService` now performs explicit attach on create and detach on drop with client-scoped logs (2026-02-11)
- [x] B3. Add Rust-side explicit dispose entrypoint (`dispose_player`) in FFI API module (2026-02-11)
- [x] B4. Wire explicit dispose API through generated FFI bindings and Dart `PlayerBridge` (2026-02-11)
- [x] B4.1 Add runtime-level `prepare_hot_restart` hook and call it during `initRustRuntime()` to evict stale clients and reset playback/output route (2026-02-11)
- [x] B5. Implement generation-based stale-client eviction policy (`prepare_hot_restart` generation rollover) and lifecycle test coverage in `runtime::tests` (2026-02-11)

## 14. Phase C Progress

- [x] C1. Plan alignment: keep host lifecycle generic and move sidecar ownership to plugin-local manager (2026-02-11)
- [x] C2. Implement plugin-local sidecar lease manager and migrate ASIO sink paths (2026-02-11)
- [x] C3. Integrate generic lifecycle-triggered cleanup hooks (no host plugin special-case): define ABI cleanup semantics (`reset`/`close`/`destroy`), enforce host `close -> destroy` for output sinks, add `runtime_shutdown`, and wire app exit path to call it before process exit (2026-02-11)
- [x] C3.1 SDK encapsulation: output sink exported `destroy` now does best-effort `close` before drop, so ABI cleanup semantic is enforced even if host skips explicit `close` (2026-02-11)
- [ ] C4. Run Hot Restart stress verification and record metrics

## 15. Phase D Progress

- [x] D1. Remove legacy per-player plugin runtime event subscription path at FFI boundary; retain runtime-global stream only (`plugin_runtime_events_global`) (2026-02-11)
- [x] D2. Remove dead audio-layer plugin runtime event hub bridge (`stellatune-audio::PluginEventHub`) and runtime bus forwarding path to engine-local hub (2026-02-11)

## 16. Out of Scope (This Refactor)

1. cross-process plugin sandboxing
2. mobile/web lifecycle unification
3. non-ASIO backend optimization unrelated to lifecycle correctness
