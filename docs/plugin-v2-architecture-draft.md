# StellaTune Plugin Runtime V2 Draft

Status: Draft  
Audience: `stellatune-plugin-api`, `stellatune-plugins`, `stellatune-audio`, `stellatune-library` maintainers  
Scope: Breaking redesign, no backward compatibility required.

## 1. Goals

1. Remove accidental concurrent calls into plugin code.
2. Decouple plugin "module" from sub-capability runtime instances.
3. Make capability instances thread-ownable (move to dedicated worker thread) without host-side global locks.
4. Provide a uniform hot config update flow (including destructive recreate).
5. Make runtime load/reload/unload safe: never unload library while any instance/in-flight call exists.

## 2. Non-Goals

1. V1 compatibility layer.
2. Cross-process sandboxing in this phase.
3. Perfect crash containment for buggy plugins (still in-process).

## 2.1 Refactor Intent (No Legacy Retention)

This redesign is an in-place replacement, not a long-term dual-stack migration.

1. During implementation we may temporarily use `V2` naming to reduce migration risk.
2. After all call sites are migrated, all V1 ABI/types/symbols/adapters must be deleted.
3. Final mainline naming should drop transitional suffixes where practical (avoid permanent `V1`/`V2` baggage).
4. Merge criteria includes repository-wide removal of legacy plugin runtime paths.

## 2.2 Implementation Status Snapshot (As of February 9, 2026)

This section is a live status marker to avoid migration confusion.

Current state:

1. Repository is in late-stage migration, with core runtime paths already switched to the new mainline API surface.
2. V2 ABI and V2 runtime execution layer exist, including typed capability instance wrappers for decoder/dsp/source/lyrics/output.
3. `PluginRuntimeService` owns native runtime management APIs (`load/reload/unload/list`) plus V2 `create_*_instance` factory APIs, and is process-singleton in backend runtime access path.
4. Backend read/query paths and source/lyrics/output execution entrypoints are partially migrated to V2 runtime.
5. Audio data-plane migration has started: DSP chain instance creation and output sink negotiate/open now use V2 runtime instances.
6. Audio query capability path (`source_list_items` / `lyrics_search/fetch` / `output_sink_list_targets`) now runs in control-thread owner actor mode with per-key instance reuse cache; caller threads use request/response messages instead of directly touching plugin instances.
7. Audio decoder selection path no longer depends on legacy `probe_best_decoder*`; it now prefers V2 module-provided extension score rules (exact match > wildcard), keeps explicit decoder selector priority, and falls back to deterministic decoder iteration when no score table is available.
8. Decoder open execution in audio now uses V2 plugin paths only (`create_decoder_instance` + instance `open_with_io`), with built-in decoder fallback kept for local files.
9. Plugin runtime event ingress/egress is now runtime-native (`stellatune_plugins::events`): backend router drains runtime events directly, and host callbacks are bound per loaded module generation with ref-counted queue lifecycle cleanup on unload/deactivate.
10. Library scan/watch support checks and metadata plugin decode extraction are now V2-driven (extension-score + V2 decoder instance `open_with_io`), removing call-heavy `PluginManager` probe/open usage from library worker hot paths.
11. Temporary legacy sync bridge (`stellatune-plugins/src/v2/sync.rs`) has been removed.
12. Audio host event publish/player-tick broadcast paths now send through runtime event bus directly (`stellatune_plugins::{push,broadcast}_shared_host_event_json`), no longer via `PluginManager` event bus.
13. Audio decode worker + preload worker + plugin reload pipeline are now V2-runtime based and no longer depend on `PluginManager`.
14. Library service bootstrap/reload path now uses V2 runtime only (legacy `start_library_with_plugins` + `PluginManager` bootstrap removed).
15. `stellatune-plugins/src/lib.rs` legacy `PluginManager`/V1 runtime containers have been removed; crate root now keeps V2 runtime + plugin install/list/uninstall surface only.
16. Remaining destructive cleanup is focused on final repository-wide V1 naming convergence and residual ABI suffix cleanup.
17. `stellatune-plugin-api` shared data structs used by V2 paths (`StIoVTable`, `StDecoderInfo`, `StOutputSinkNegotiatedSpec`) are now de-V1-suffixed and call sites were updated.
18. Root API constants were renamed to mainline names (`STELLATUNE_PLUGIN_API_VERSION`, `STELLATUNE_PLUGIN_ENTRY_SYMBOL`) and SDK/plugin call sites were updated.
19. Built-in plugins (`asio`/`netease`/`ncm`) use the unified `export_plugin!` path with capability instance adapters.
20. SDK legacy plugin-entry export chain (`macros/export_plugin.rs`) has been removed, and host helper path now resolves host callbacks through V2 host vtable.
21. Residual V1 optional-interface ABI (`StSourceCatalog*V1` / `StLyricsProviderVTableV1` / `StOutputSink*V1`) and SDK helper macros (`export_source_catalogs_interface!`, `export_output_sinks_interface!`, `compose_get_interface!`) were removed.
22. ABI naming convergence phase-1 is complete: ABI structs/constants now use mainline names (no `V2` suffix), and host/runtime/SDK/plugin call sites were updated accordingly.
23. Naming convergence phase-2 is complete: runtime-side capability wrapper/service/event/load/report type names and shared runtime entrypoints were converged to mainline names; SDK config-update + descriptor/helper surfaces were simplified to mainline names; plugin instance adapter local `*V2` names were removed.
24. Remaining cleanup is mainly documentation wording and final acceptance-criteria closure work.
25. High-ROI lock optimization is in place for runtime read-mostly structures: `PluginRuntimeService` plugin slot/module maps and `CapabilityRegistry` now use `RwLock` with read/write path separation.
26. Config-update runtime orchestration (stage-1) is now wired in `stellatune-plugins`: per-instance update requests track decision (`HotApply/Recreate/Reject`) and terminal outcome (`Applied/RequiresRecreate/Rejected/Failed`), and capability wrappers now return structured update results instead of fire-and-forget apply.
27. Audio control-thread owner path now applies structured config updates for runtime query capability instances (`source_list_items` / `lyrics_search/fetch` / `output_sink_list_targets`): when config changes it attempts `HotApply`, falls back to owner-thread `Recreate` with optional state migration, and reports deterministic reject/fail errors.
28. Active output-sink data-plane worker now supports owner-thread config hot update: when route identity (`plugin/type/target/spec`) is unchanged it first attempts `HotApply`, and automatically falls back to safe worker recreate when update fails or requires recreate.
29. Active decode-thread DSP chain path now runs owner-thread update orchestration: control plane sends normalized DSP chain specs, decode thread applies in-place `HotApply` when shape is stable, and falls back to per-node recreate or full chain recreate when required.
30. Active decoder path now supports owner-thread refresh/recreate on plugin reload: control plane issues a decode-thread `RefreshDecoder` command, decode thread reopens decoder from runtime and seeks back to current playback position at a safe boundary.
31. Library metadata extraction now uses per-worker-thread decoder instance ownership with bounded idle cache: candidate generation/config drift is handled via structured `HotApply/Recreate` fallback, stale entries are evicted by TTL/cap policy, and library plugin reload paths clear worker-side decoder caches before reload to reduce draining-generation residue.
32. Runtime update outcome visibility has started: control-thread query capability config updates now emit runtime `notify` events (`topic=host.instance.config_update`) with capability/type/status/generation/detail so backend/FFI/UI subscribers can render explicit update/recreate outcomes.
33. Runtime update outcome visibility is extended to playback data-plane paths: decode-thread DSP updates and output sink worker config updates now emit the same `host.instance.config_update` notify payloads for applied/recreate/rejected/failed outcomes.
34. Runtime update outcome visibility is now user-readable in Settings runtime debug: Flutter parses `host.instance.config_update` payloads into structured lines (`capability/type -> status, generation, detail`) instead of raw JSON blobs.
35. Audio runtime notify emission has been consolidated into a shared engine helper, and recreate-failure branches in query/output/DSP update paths now emit explicit `failed` status events to keep HotApply/Recreate telemetry normalized.

## 3. High-Level Model

V2 separates three layers:

1. Module layer (`PluginModule` under `stellatune_plugin_api`): metadata + capability factories only.
2. Capability descriptor/factory layer: discoverable capability types and create instance.
3. Instance layer: stateful runtime object with its own VTable.

Design rule:

1. Module object does not own business-state instances.
2. Instances are independent objects returned by factory APIs.
3. Host controls instance scheduling; data plane calls happen on chosen worker thread.

## 4. Instance Mobility and Concurrency Contract

V2 uses one unified runtime contract:

1. All capability instances are required to be movable across threads (Send-like semantics).
2. Host guarantees per-instance exclusive call execution (no concurrent calls to the same instance).
3. Concurrency policy is a host runtime concern, not an ABI enum in this phase.

This keeps plugin authoring simple:

1. Plugin instances can be created on one thread and moved to decode/output/library worker threads.
2. Plugin code does not need to implement internal synchronization unless plugin author chooses to.

## 5. Module and Capability ABI Sketch

```rust
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StCapabilityKind {
    Decoder = 1,
    Dsp = 2,
    SourceCatalog = 3,
    LyricsProvider = 4,
    OutputSink = 5,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct StCapabilityDescriptor {
    pub kind: StCapabilityKind,
    pub type_id_utf8: StStr,
    pub display_name_utf8: StStr,
    pub config_schema_json_utf8: StStr,
    pub default_config_json_utf8: StStr,
    pub reserved0: u32,
    pub reserved1: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct StPluginModule {
    pub api_version: u32,
    pub plugin_version: StVersion,
    pub metadata_json_utf8: extern "C" fn() -> StStr,

    pub capability_count: extern "C" fn() -> usize,
    pub capability_get: extern "C" fn(index: usize) -> *const StCapabilityDescriptor,

    pub decoder_ext_score_count: Option<extern "C" fn(type_id_utf8: StStr) -> usize>,
    pub decoder_ext_score_get: Option<
        extern "C" fn(type_id_utf8: StStr, index: usize) -> *const StDecoderExtScore,
    >,

    pub create_decoder_instance: Option<
        extern "C" fn(
            type_id_utf8: StStr,
            config_json_utf8: StStr,
            out_instance: *mut StDecoderInstanceRef,
        ) -> StStatus,
    >,
    pub create_dsp_instance: Option<
        extern "C" fn(
            type_id_utf8: StStr,
            sample_rate: u32,
            channels: u16,
            config_json_utf8: StStr,
            out_instance: *mut StDspInstanceRef,
        ) -> StStatus,
    >,
    pub create_source_catalog_instance: Option<
        extern "C" fn(
            type_id_utf8: StStr,
            config_json_utf8: StStr,
            out_instance: *mut StSourceCatalogInstanceRef,
        ) -> StStatus,
    >,
    pub create_lyrics_provider_instance: Option<
        extern "C" fn(
            type_id_utf8: StStr,
            config_json_utf8: StStr,
            out_instance: *mut StLyricsProviderInstanceRef,
        ) -> StStatus,
    >,
    pub create_output_sink_instance: Option<
        extern "C" fn(
            type_id_utf8: StStr,
            config_json_utf8: StStr,
            out_instance: *mut StOutputSinkInstanceRef,
        ) -> StStatus,
    >,

    // Optional plugin-wide cleanup hook before module is finally dropped.
    pub shutdown: Option<extern "C" fn() -> StStatus>,
}
```

Decoder extension score rule shape:

```rust
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StDecoderExtScore {
    pub ext_utf8: StStr, // lowercase extension without dot, "*" as wildcard
    pub score: u16,      // higher is preferred
    pub flags: u16,      // reserved
    pub reserved: u32,
}
```

Selection rule in this phase:

1. Only extension-hint based scoring is used for decoder ordering.
2. No metadata/header probing is used in host selection path.
3. Plugin may provide wildcard (`*`) fallback when exact extension is not present.
4. ABI version must be bumped when V2 ABI layout changes (current draft implementation: `api_version = 5`).

Entry symbol:

```rust
pub type StPluginEntry =
    unsafe extern "C" fn(host: *const StHostVTable) -> *const StPluginModule;
```

## 6. Instance ABI Pattern

Each capability has:

1. `InstanceRef` = `{ handle, vtable }`
2. `InstanceVTable` = runtime methods + config update + destroy.

Example shape:

```rust
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct StDecoderInstanceRef {
    pub handle: *mut core::ffi::c_void,
    pub vtable: *const StDecoderInstanceVTable,
    pub reserved0: u32,
    pub reserved1: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StConfigUpdateMode {
    HotApply = 1,
    Recreate = 2,
    Reject = 3,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct StConfigUpdatePlan {
    pub mode: StConfigUpdateMode,
    pub reason_utf8: StStr, // optional
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct StDecoderInstanceVTable {
    pub open: extern "C" fn(handle: *mut c_void, args: StDecoderOpenArgs) -> StStatus,
    pub get_info: extern "C" fn(handle: *mut c_void, out: *mut StDecoderInfo) -> StStatus,
    pub read_interleaved_f32: extern "C" fn(
        handle: *mut c_void,
        frames: u32,
        out_interleaved: *mut f32,
        out_frames_read: *mut u32,
        out_eof: *mut bool,
    ) -> StStatus,
    pub seek_ms: Option<extern "C" fn(handle: *mut c_void, position_ms: u64) -> StStatus>,

    pub plan_config_update_json_utf8: Option<
        extern "C" fn(
            handle: *mut c_void,
            new_config_json_utf8: StStr,
            out_plan: *mut StConfigUpdatePlan,
        ) -> StStatus,
    >,
    pub apply_config_update_json_utf8: Option<
        extern "C" fn(handle: *mut c_void, new_config_json_utf8: StStr) -> StStatus,
    >,

    pub export_state_json_utf8:
        Option<extern "C" fn(handle: *mut c_void, out_json_utf8: *mut StStr) -> StStatus>,
    pub import_state_json_utf8:
        Option<extern "C" fn(handle: *mut c_void, state_json_utf8: StStr) -> StStatus>,

    pub destroy: extern "C" fn(handle: *mut c_void),
}
```

```rust
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct StDecoderOpenArgs {
    pub path_utf8: StStr,
    pub ext_utf8: StStr,
    pub io_vtable: *const StIoVTable,
    pub io_handle: *mut c_void,
}
```

Same pattern applies to DSP/SourceCatalog/LyricsProvider/OutputSink instance VTables.

## 7. Config Hot Update Protocol

Unified host protocol:

1. Host calls `plan_config_update`.
2. If `HotApply`: call `apply_config_update` in place.
3. If `Recreate`:
   1. Create new instance with new config.
   2. If supported, `export_state(old)` then `import_state(new)`.
   3. Swap active instance at safe boundary.
   4. Destroy old instance.
4. If `Reject`: keep old instance, surface reason.

Audio-safe swap boundary recommendation:

1. Decoder: between read iterations.
2. DSP: block/frame boundary.
3. OutputSink: flush + reopen + transition fade.
4. Source stream: reopen and reseek by logical offset if possible.

### 7.1 Recommended Implementation: ArcSwap + Actor

Use a hybrid model:

1. `HotApply` path uses `ArcSwap` for lock-free config snapshot switching.
2. `Recreate` path uses the instance owner thread (actor) for lifecycle-safe replacement.

Per-instance runtime fields (host-side, conceptual):

1. `params: ArcSwap<ParamBlock>` for read-mostly runtime parameters.
2. `config_gen: AtomicU64` for monotonic config generation.
3. `instance_state` owned by actor thread for handles/resources.

`HotApply` flow:

1. Build a validated immutable `ParamBlock` from incoming config.
2. `params.store(Arc::new(new_block))`.
3. `config_gen.fetch_add(1, Ordering::Release)`.
4. Data plane reads `let p = params.load();` at block boundary and uses that snapshot for the whole block.

`Recreate` flow:

1. Control plane sends `Recreate { new_config, target_gen }` command to actor.
2. Actor creates new instance and optionally performs warmup.
3. Actor optionally migrates state (`export_state` old -> `import_state` new).
4. Actor swaps active instance at safe boundary.
5. Actor destroys old instance.
6. Actor publishes result (`Applied/Recreated/Rejected`) and final generation.

Important boundaries:

1. `ArcSwap` solves atomic parameter publication only.
2. `ArcSwap` does not solve resource-handle recreation, per-instance serialization, or unload safety.
3. Therefore, `ArcSwap` must be paired with actor-based lifecycle control.

### 7.2 Host-Side Pseudocode (Rust)

```rust
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use arc_swap::ArcSwap;

#[derive(Clone)]
struct ParamBlock {
    // Immutable runtime parameters used by data plane.
}

struct InstanceRuntime {
    params: ArcSwap<ParamBlock>,
    config_gen: AtomicU64,
    cmd_tx: crossbeam_channel::Sender<InstanceCmd>,
}

enum InstanceCmd {
    Recreate { new_config_json: String, target_gen: u64 },
    Shutdown,
}

enum UpdateOutcome {
    Applied { gen: u64 },
    Recreated { gen: u64 },
    Rejected { reason: String, gen: u64 },
}
```

Control-plane update entry:

```rust
fn update_instance_config(rt: &InstanceRuntime, new_config_json: &str) -> anyhow::Result<UpdateOutcome> {
    match plan_update_mode(new_config_json)? {
        StConfigUpdateMode::HotApply => {
            let block = build_param_block(new_config_json)?;
            rt.params.store(Arc::new(block));
            let gen = rt.config_gen.fetch_add(1, Ordering::AcqRel) + 1;
            Ok(UpdateOutcome::Applied { gen })
        }
        StConfigUpdateMode::Recreate => {
            let next_gen = rt.config_gen.load(Ordering::Acquire) + 1;
            rt.cmd_tx.send(InstanceCmd::Recreate {
                new_config_json: new_config_json.to_string(),
                target_gen: next_gen,
            })?;
            // Caller waits for actor ack/event and returns Recreated/Rejected.
            wait_recreate_result(next_gen)
        }
        StConfigUpdateMode::Reject => {
            let gen = rt.config_gen.load(Ordering::Acquire);
            Ok(UpdateOutcome::Rejected {
                reason: "plugin rejected update".to_string(),
                gen,
            })
        }
    }
}
```

Data-plane usage:

```rust
fn process_block(rt: &InstanceRuntime /*, audio buffers... */) {
    let params = rt.params.load();
    // Use this snapshot for the entire block.
    run_with_params(&params);
}
```

Actor-side recreate orchestration:

```rust
fn handle_recreate(active: &mut PluginInstance, cfg: String, target_gen: u64) -> anyhow::Result<()> {
    let mut next = create_instance(&cfg)?;
    if let Some(state) = active.export_state_json().ok() {
        let _ = next.import_state_json(&state);
    }
    warmup_if_needed(&mut next)?;
    // Swap at safe boundary decided by capability worker.
    let old = std::mem::replace(active, next);
    old.destroy();
    publish_recreate_ok(target_gen);
    Ok(())
}
```

Notes:

1. `config_gen` is host-observed generation, useful for ack correlation and stale-update detection.
2. `ArcSwap` path must never directly destroy/recreate resource handles.
3. Recreate result should be surfaced through actor response/event, not inferred synchronously.

## 8. Runtime Load/Reload/Unload Safety

Core invariant:

1. A dynamic library can be unloaded only when:
   1. `live_instances == 0`
   2. `inflight_calls == 0`
   3. no host object can still reach that generation.

Recommended host data model:

1. `PluginId` -> `PluginSlot`.
2. `PluginSlot` contains generations:
   1. `Active(gen_n)`
   2. zero or more `Draining(gen_old...)`.

Generation behavior:

1. New load/reload creates `gen_n+1` and marks active.
2. New instances can only be created from active generation.
3. Old generation enters `Draining`.
4. Background reaper unloads old generation only when invariants hold.

Each instance handle in host stores:

1. strong ref to generation lifetime guard.
2. capability type id.
3. runtime-side scheduler metadata (optional, host internal).

This ensures no `dlclose` while any instance exists.

## 9. Locking Strategy

No host global mutex on data plane calls.

Use only:

1. Runtime control-plane serialization for load/reload/unload state transitions (actor or single control thread).
2. Optional per-instance mailbox when host wants explicit thread ownership handoff.
3. Read-mostly runtime indexes/registries should prefer `RwLock` over `Mutex` when write frequency is low and read-side contention dominates.

Data plane (decode/process/write/search) runs directly on owning worker thread.

`ArcSwap` usage guidance:

1. Suitable for read-mostly immutable parameter snapshots (`ParamBlock`).
2. Not a replacement for actor-based instance recreate/destroy orchestration.

## 10. Recommended Host Architecture

1. `PluginRuntimeService` actor:
   1. load/unload/reload
   2. descriptor query
   3. instance create/destroy
   4. config update orchestration
2. Capability workers:
   1. decode thread owns decoder instances
   2. output thread owns sink instances
   3. library scan worker owns metadata decoder instances
3. Handle API:
   1. typed instance handles are always movable across threads
   2. host runtime enforces single active caller per instance

## 11. SDK Implications

`stellatune-plugin-sdk` should generate:

1. capability descriptors without per-instance threading enum.
2. instance factories returning `InstanceRef`.
3. default config update behavior:
   1. if no update hook, return `Recreate`
   2. if plugin opts in, provide hot apply.

SDK trait direction:

1. split descriptor/factory trait and instance trait.
2. add optional state migration trait for recreate path.

## 12. Failure/Edge Cases

1. Host crash on plugin UB is still possible (in-process constraint).
2. `destroy` must be idempotent on host side (guard against double-drop paths).
3. If `apply_config_update` fails, old instance remains active.
4. If recreate swap fails mid-way:
   1. destroy new instance
   2. keep old instance
   3. emit runtime error event.

## 13. Code Organization and Maintainability Requirements

V2 must be implemented with long-term readability/maintainability as a hard requirement.

1. Do not collapse the new runtime and ABI logic into a single large file.
2. Split by responsibility and lifecycle boundary, not by "misc helpers".
3. Keep API types, runtime orchestration, and capability-specific execution in separate modules.
4. Prefer small, focused modules with clear ownership and minimal cross-module coupling.
5. Introduce shared utility modules only for truly shared concerns (avoid "god util" files).
6. Keep naming consistent across crates (`api`, `sdk`, `runtime`, `capability`, `instance`, `update`, `lifecycle`).
7. Every unsafe block should live close to its invariant explanation and not be hidden in unrelated files.

Suggested decomposition (illustrative):

1. `stellatune-plugin-api`:
   1. `abi/mod.rs` (public surface)
   2. `abi/common.rs` (StStr/StStatus/common enums)
   3. `abi/module.rs` (module + capability descriptors)
   4. `abi/instance/*.rs` (decoder/dsp/source/lyrics/output instance refs + vtables)
2. `stellatune-plugins`:
   1. `runtime/mod.rs`
   2. `runtime/lifecycle.rs` (generation load/reload/unload)
   3. `runtime/instance_registry.rs`
   4. `runtime/update.rs` (HotApply/Recreate orchestration)
   5. `capabilities/*.rs` (typed host wrappers)
3. `stellatune-plugin-sdk`:
   1. descriptor/factory macros separate from instance method shims
   2. config-update helpers separate from codec/data-plane helpers

Review checklist for PRs:

1. New file responsibilities are explicit in module docs/comments.
2. No single file becomes the implicit center for all runtime logic.
3. Public API changes and runtime behavior changes are not mixed without clear structure.

## 14. Incremental Implementation Plan (No Compatibility)

Status legend: `DONE`, `IN_PROGRESS`, `NOT_STARTED`.

1. `DONE` Introduce new ABI structs in `stellatune-plugin-api` (temporary `V2` names allowed during migration).
2. `DONE` Implement SDK codegen against the new ABI only.
Current: SDK export path is mainline (`export_plugin!`), built-in plugins are migrated, and residual V1 optional-interface helper macros/types were removed.
3. `DONE` Introduce host runtime generation manager in `stellatune-plugins`.
Current: `PluginRuntimeService` includes native `load/reload/unload/list` management path, shared singleton access, and typed `create_*_instance` V2 execution APIs.
4. `DONE` Remove `PluginManager: Clone` usage in call-heavy paths.
Current: audio/library hot paths are V2 runtime only, and legacy `PluginManager` container code was removed from `stellatune-plugins` root.
5. `DONE` Migrate `stellatune-audio` decode/output pipeline to instance-owner model.
Current: DSP and output sink execution use V2 instances; source/lyrics/output query is now owner-actor based with instance reuse; output sink worker + decode-thread DSP chain now support owner-thread config update/recreate flow; reload path now forces active output sink generation rebind and decode promoted-preload cache invalidation; decoder selection is extension-score based and no longer uses legacy probe scoring; decoder open in audio now uses V2 source/decoder instances for plugin paths with built-in local fallback.
6. `DONE` Migrate `stellatune-library` metadata scan/watch to instance-owner model.
Current: scan/watch support checks and metadata plugin decode are runtime-driven; metadata decoder instances are owner-thread cached with generation/config-aware update/recreate fallback; service-layer plugin bootstrap/reload is runtime-only.
7. `DONE` Delete all V1 ABI/types/symbols/adapters and old plugin runtime paths.
Current: `stellatune-plugins` legacy manager/runtime paths are deleted; legacy host/plugin entry ABI and old SDK `export_plugin!` path are removed; optional source/lyrics/output interface ABI + helper macros are deleted; repository code paths no longer contain V1 ABI symbols.
8. `DONE` Remove temporary migration suffixes and keep only the new mainline API surface.
Current: host/plugin entry symbols and ABI structs/constants are converged to mainline names; runtime/service/helper and SDK descriptor/helper naming convergence is complete; plugin instance adapter local `*V2` names are removed.
9. `DONE` Remove/avoid broad `unsafe impl Sync` on runtime containers.
Current: legacy broad `unsafe impl Sync` containers were removed with V1 container deletion; V2 instance wrappers remain `Send`-only.
10. `IN_PROGRESS` Enforce modular file layout and readability constraints during migration.
Current: runtime code is split by concerns; shared runtime update notify logic is extracted into engine-level helper module, and ongoing refactor should continue tightening file/module boundaries.
11. `DONE` Implement unified HotApply/Recreate orchestration.
Current: runtime update coordinator + capability wrappers expose structured per-instance update outcomes; audio control-thread query-instance owner path, active output sink worker path, decode-thread DSP chain path, and library metadata decoder worker path perform `HotApply/Recreate`; runtime notify visibility is normalized across applied/requires_recreate/recreated/rejected/failed, including recreate-failure branches.

### 14.1 Next Refactor Plan (From Current State)

1. Stage A: Decoder path migration in audio worker
   1. `DONE` Introduce decoder selection strategy (replace legacy `probe_best_decoder*` dependency) using extension-score callbacks.
   2. `DONE` Migrate `open_engine_decoder` and related decode entrypoints to instance factories.
   Current: plugin decode open path is runtime-only (including source stream path via source instance `open_stream`); built-in decoder fallback is preserved for local files.
   3. Keep built-in decoder fallback behavior unchanged while replacing plugin decoder selection/open.
2. Stage B: Library worker migration
   1. `DONE` Migrate metadata/scan/watch decode capability checks and decoder open calls to V2 runtime.
   2. `DONE` Remove library-side `PluginManager` snapshot/clone dependency in worker hot paths.
3. Stage C: Runtime event path migration
   1. `DONE` Move plugin host event ingress/egress from legacy manager helpers to V2 runtime-native path.
   2. `DONE` Ensure reload/deactivate keeps actor-owned instance and event state consistent.
   Current: V2 host callback context is owned per loaded module generation; event queues are ref-counted by plugin id and automatically cleaned after last generation unloads, avoiding stale queue/event leakage across reload/deactivate.
4. Stage D: Legacy deletion gate
   1. `DONE` Delete V1 runtime execution paths in `stellatune-plugins` after audio/library call sites were migrated.
   2. `DONE` Remove broad legacy `unsafe impl Sync` containers together with V1 deletion.
   3. `DONE` Rename transition-only `V2` surfaces where appropriate to become the mainline API.
   Current: plugin implementations are migrated to `export_plugin!`; V1 ABI/runtime surfaces are deleted; naming/API convergence is completed in code.
5. Stage E: Config update execution completion
   1. `DONE` Introduce runtime-side structured update coordination (`Applied/RequiresRecreate/Rejected/Failed`) for all capability wrappers.
   2. `DONE` Wire owner-thread `Recreate` swap paths for decode/output/library workers at safe boundaries.
   Current: control-thread query capability instances (`source/lyrics/output-targets`), active output sink worker (including generation-aware reload rebind), decode-thread DSP chain, reload-triggered active decoder recreate path, and library metadata worker decoder path are wired; recreate failure branches now emit explicit `failed` runtime notify status.
   3. `DONE` Expose update outcome/status via backend/FFI/UI so users can observe hot-apply/recreate results explicitly.
   Current: query capability + output sink worker + decode DSP + decoder refresh paths emit unified `host.instance.config_update` notify payloads; backend/FFI streams are wired and Settings runtime debug now renders these payloads as structured status lines.
   Validation snapshot: `cargo test -p stellatune-plugins` and `cargo test -p stellatune-backend-api` pass; `cargo test -p stellatune-audio` currently has no test cases. Manual E2E verification for playback-time DSP/output hot-update and reload/uninstall behavior is still required for final acceptance closure.

## 15. Open Questions

1. Do we need explicit host-side cancellation API for long-running instance methods?
2. Should config update plan include cost hint (`low/medium/high`)?
3. Should `export_state/import_state` use JSON only or allow binary blob (`StSlice<u8>`)?
4. Whether to enforce hard timeout for `destroy` callbacks.

## 16. Acceptance Criteria for V2

Status legend: `PASS`, `PARTIAL`, `PENDING`.

1. `PARTIAL` No dynamic library unload while any instance from that generation exists.
Current: lifecycle primitives exist; full guarantee awaits V2-native load/unload path replacing legacy manager execution.
2. `PARTIAL` No implicit concurrent calls to the same instance.
Current: design and runtime structures are in place; full enforcement depends on audio/library worker migration completion.
3. `PARTIAL` Hot config update path works for at least one DSP and one output sink plugin.
Current: runtime orchestration + structured outcomes are implemented; owner-thread query-instance hot-update, active output sink worker hot-update/recreate fallback, and library metadata worker decoder update/recreate path are wired. Backend/FFI/UI observable update status is in place; concrete playback-time DSP/output sink E2E validation remains pending.
4. `PASS` Decode/output/library workers no longer depend on cloning whole plugin runtime state.
5. `PENDING` Per-instance serialization violations in host runtime are detected and surfaced as deterministic errors.
