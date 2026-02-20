# Wasm Plugin System Replacement Plan (Breaking Change)

This plan replaces the current dynamic-library plugin system with a Wasm
component-based system. Backward compatibility with old dynamic plugins is not
required.

## 1. Scope and Decisions

- Replace `crates/stellatune-plugins` runtime loading path for production use.
- Use WIT contract as the only host/plugin ABI (`wit/stellatune-plugin/*.wit`).
- Keep one installable plugin package id, but allow many component binaries per
  package via manifest (`plugin.json`).
- Build each ability as an independent component:
  - decoder
  - source
  - lyrics
  - output-sink
  - dsp
- Add optional hot-path core module negotiation for:
  - output-sink
  - dsp
- Follow ABI spec in `docs/hot-path-core-abi.md`.
- Remove `-pure` suffix naming; base world means no sidecar import.
- Host owns sidecar lifecycle and transport allocation.

## 2. New Crate Layout

Create a new crate: `crates/stellatune-wasm-plugins`.

Initial modules:

- `manifest`
  - parse/validate `plugin.json`
  - normalize component paths and ability routing table
- `installer`
  - artifact unpack/copy
  - receipt writing for Wasm package format
- `runtime`
  - component loading
  - world validation
  - instance lifecycle
- `host_imports`
  - `host-stream`, `http-client`, `sidecar` implementation
- `capability`
  - decoder endpoint bridge
  - source endpoint bridge
  - lyrics endpoint bridge
- `scheduler`
  - dedicated thread vs shared pool execution
- `errors`
  - canonical error mapping between host errors and WIT `plugin-error`

## 3. Target Runtime Model

- Install unit: plugin package id (one directory).
- Activation unit: component id from manifest.
- Invocation route key:
  `(plugin_id, component_id, ability_kind, type_id)`.
- Threading:
  - `dedicated`: one worker per component instance group.
  - `shared_pool`: scheduled on named pool.
- Sidecar:
  - component-scoped process registry.
  - hard terminate during unload/shutdown.
- Lifecycle states:
  - `Discovered -> Loaded -> Active -> Quiescing -> Unloaded`.
- Lifecycle hooks:
  - host invokes `lifecycle.on-enable()` after activation.
  - host invokes `lifecycle.on-disable(reason)` before disable/unload/shutdown.

## 4. Manifest and Artifact Contract

Manifest file: `plugin.json` in plugin root.

Required fields:

- `schema_version`
- `id`
- `version`
- `api_version`
- `components[]` with `id`, `path`, `world`, `abilities[]`

Validation rules:

- `id` must be stable and non-empty.
- every component `path` must exist and stay inside plugin root.
- `(kind, type_id)` cannot collide within the same plugin package.
- declared world must match component exports/imports.

Receipts:

- Replace single `library_rel_path` with component table snapshot.
- Keep install/uninstall marker behavior from current installer.

## 5. Phased Implementation

## Phase 0: Contract Freeze

- Finalize WIT names and worlds in `wit/stellatune-plugin/worlds.wit`.
- Finalize manifest schema in `docs/wasm-plugin-manifest.md`.
- Define error codes and logging policy.

Exit criteria:

- WIT + manifest reviewed and accepted.

## Phase 1: New Crate Bootstrap

- Add `crates/stellatune-wasm-plugins/Cargo.toml`.
- Add empty modules with compile-only scaffolding.
- Wire crate into workspace `Cargo.toml`.

Exit criteria:

- workspace builds with new crate.

## Phase 2: Manifest + Installer

- Implement manifest structs and serde parsing.
- Implement secure artifact extraction and root normalization.
- Implement `install/list/uninstall` for Wasm plugin package layout.
- Add unit tests for malformed paths, duplicate abilities, missing files.

Exit criteria:

- install/list/uninstall works end-to-end for Wasm package fixtures.

## Phase 3: Runtime Core + Component Loading

- Introduce Wasm engine integration (component model).
- Implement component load cache keyed by `(plugin_id, component_id, hash)`.
- Implement world verification against manifest `world`.
- Implement plugin activation/deactivation and unload flow.

Exit criteria:

- runtime can load/unload component package with no capability calls.

## Phase 4: Host Imports

- Implement `host-stream` import backed by existing track/source I/O.
- Implement `http-client` import with host-side policy/timeouts.
- Implement `sidecar` import:
  - launch
  - open control/data channels
  - transport negotiation
  - lifecycle cleanup

Exit criteria:

- integration tests pass for stream read/seek, http fetch, sidecar echo.

## Phase 4.5: Lifecycle Hook Orchestration

- Add lifecycle dispatcher in runtime:
  - `on-enable` at activation boundary
  - `on-disable(host-disable|unload|shutdown|reload)` at teardown boundaries
- Add timeout policy for lifecycle hooks (e.g. soft timeout + forced cleanup).
- Define hook failure behavior:
  - `on-enable` failure prevents ability registration for that component.
  - `on-disable` failure is logged and followed by forced teardown.
- Add tests for sidecar components:
  - sidecar launched only after `on-enable`
  - sidecar terminated on `on-disable`

Exit criteria:

- lifecycle hooks are deterministic and enforced for all component worlds.

## Phase 5: Capability Bridges

- Implement decoder bridge to existing decode worker endpoint style.
- Implement source bridge to existing source worker endpoint style.
- Implement lyrics bridge to existing lyrics worker endpoint style.
- Implement sink/dsp hot-path bridge using `describe-hot-path(...)` and core
  module pointer-length ABI (with fallback to component calls).
- Add typed metadata mapping to backend models.

Exit criteria:

- host can discover and invoke decoder/source/lyrics from Wasm components.

## Phase 6: Scheduler and Isolation

- Implement dedicated worker model per component.
- Implement shared pools (`io`, `cpu`) for light workloads.
- Add per-component limits (`max_instances`) and backpressure behavior.
- Add shutdown ordering and timeout handling.

Exit criteria:

- components can run in different threads as directed by manifest.

## Phase 7: App/Backend Cutover

- Replace backend runtime calls from old plugin crate to new crate.
- Replace plugin list/install APIs to read new manifest format.
- Keep old APIs removed or failing fast with clear error.

Exit criteria:

- app boots and uses Wasm plugin runtime only.

## Phase 8: Plugin Migration

- Port one decoder plugin first (reference implementation).
- Port one sidecar-heavy plugin (ASIO-style pattern) second.
- Port one network/lyrics plugin third.
- Publish migration guide for plugin authors.

Exit criteria:

- at least 3 migrated plugins pass runtime tests.

## Phase 9: Remove Legacy Runtime

- Delete legacy dynamic plugin loading paths in `crates/stellatune-plugins`.
- Remove old FFI/plugin ABI crates if no longer used.
- Clean backend/UI references to legacy metadata fields.

Exit criteria:

- no build/runtime dependency on old dynamic plugin loader.

## 6. Testing Strategy

- Unit tests:
  - manifest parsing and validation
  - installer path safety
  - ability routing collision checks
- Integration tests:
  - load/unload lifecycle
  - decoder/source/lyrics calls
  - sidecar launch and transport fallbacks
- Stress tests:
  - reload loops
  - concurrent component instances
  - sidecar crash/restart handling
- Regression tests:
  - track switch under active decode
  - app shutdown while sidecar busy

## 7. Risks and Mitigations

- Risk: sidecar resource leaks on component drop.
  - Mitigation: central process registry + hard timeout termination.
- Risk: capability latency regressions from Wasm boundary.
  - Mitigation: keep control-plane in calls, move large data to chunked read or
    shared memory transport.
- Risk: plugin packaging mistakes.
  - Mitigation: strict manifest validation and deterministic error messages.
- Risk: threading model mismatch with host workers.
  - Mitigation: explicit scheduler module and per-component policy tests.

## 8. Breaking Change Rollout Checklist

- Add runtime feature flag: `wasm_plugins_only`.
- Cut a version boundary and migration note in release docs.
- Reject old artifacts at install with actionable error text.
- Document new package format and world naming for plugin authors.

## 9. Immediate Next Actions

1. Create `crates/stellatune-wasm-plugins` scaffolding and workspace wiring.
2. Implement manifest structs from `docs/wasm-plugin-manifest.md`.
3. Switch installer to require `plugin.json` and multi-component layout.
4. Add fixture plugins for decoder/source/lyrics and sidecar transport tests.
