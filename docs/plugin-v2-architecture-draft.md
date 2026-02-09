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

1. Repository is in a transitional phase, not final V2-only state.
2. V2 ABI and V2 runtime execution layer exist, including typed capability instance wrappers for decoder/dsp/source/lyrics/output.
3. `PluginRuntimeService` owns native runtime management APIs (`load/reload/unload/list`) plus V2 `create_*_instance` factory APIs, and is process-singleton in backend runtime access path.
4. Backend read/query paths and source/lyrics/output execution entrypoints are partially migrated to V2 runtime.
5. Audio data-plane migration has started: DSP chain instance creation and output sink negotiate/open now use V2 runtime instances.
6. Audio query capability path (`source_list_items` / `lyrics_search/fetch` / `output_sink_list_targets`) now runs in control-thread owner actor mode with per-key instance reuse cache; caller threads use request/response messages instead of directly touching plugin instances.
7. Audio decoder selection path no longer depends on legacy `probe_best_decoder*`; it now prefers V2 module-provided extension score rules (exact match > wildcard), keeps explicit decoder selector priority, and falls back to deterministic decoder iteration when no score table is available.
8. Decoder open execution in audio is now V2-first (`create_decoder_instance` + instance `open_with_io`) with legacy V1 fallback for not-yet-migrated plugins/source paths; plugin runtime event bus still retains legacy V1 manager usage.
9. Temporary legacy->V2 sync bridge (`stellatune-plugins/src/v2/sync.rs`) has been removed.
10. Final target remains unchanged: delete all legacy runtime/ABI paths after call-site migration completes.

## 3. High-Level Model

V2 separates three layers:

1. Module layer (`PluginModuleV2`): metadata + capability factories only.
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
pub enum StCapabilityKindV2 {
    Decoder = 1,
    Dsp = 2,
    SourceCatalog = 3,
    LyricsProvider = 4,
    OutputSink = 5,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct StCapabilityDescriptorV2 {
    pub kind: StCapabilityKindV2,
    pub type_id_utf8: StStr,
    pub display_name_utf8: StStr,
    pub config_schema_json_utf8: StStr,
    pub default_config_json_utf8: StStr,
    pub reserved0: u32,
    pub reserved1: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct StPluginModuleV2 {
    pub api_version: u32,
    pub plugin_version: StVersion,
    pub metadata_json_utf8: extern "C" fn() -> StStr,

    pub capability_count: extern "C" fn() -> usize,
    pub capability_get: extern "C" fn(index: usize) -> *const StCapabilityDescriptorV2,

    pub decoder_ext_score_count: Option<extern "C" fn(type_id_utf8: StStr) -> usize>,
    pub decoder_ext_score_get: Option<
        extern "C" fn(type_id_utf8: StStr, index: usize) -> *const StDecoderExtScoreV2,
    >,

    pub create_decoder_instance: Option<
        extern "C" fn(
            type_id_utf8: StStr,
            config_json_utf8: StStr,
            out_instance: *mut StDecoderInstanceRefV2,
        ) -> StStatus,
    >,
    pub create_dsp_instance: Option<
        extern "C" fn(
            type_id_utf8: StStr,
            sample_rate: u32,
            channels: u16,
            config_json_utf8: StStr,
            out_instance: *mut StDspInstanceRefV2,
        ) -> StStatus,
    >,
    pub create_source_catalog_instance: Option<
        extern "C" fn(
            type_id_utf8: StStr,
            config_json_utf8: StStr,
            out_instance: *mut StSourceCatalogInstanceRefV2,
        ) -> StStatus,
    >,
    pub create_lyrics_provider_instance: Option<
        extern "C" fn(
            type_id_utf8: StStr,
            config_json_utf8: StStr,
            out_instance: *mut StLyricsProviderInstanceRefV2,
        ) -> StStatus,
    >,
    pub create_output_sink_instance: Option<
        extern "C" fn(
            type_id_utf8: StStr,
            config_json_utf8: StStr,
            out_instance: *mut StOutputSinkInstanceRefV2,
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
pub struct StDecoderExtScoreV2 {
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
4. ABI version must be bumped when V2 ABI layout changes (current draft implementation: `api_version = 4`).

Entry symbol:

```rust
pub type StPluginEntryV2 =
    unsafe extern "C" fn(host: *const StHostVTableV2) -> *const StPluginModuleV2;
```

## 6. Instance ABI Pattern

Each capability has:

1. `InstanceRef` = `{ handle, vtable }`
2. `InstanceVTable` = runtime methods + config update + destroy.

Example shape:

```rust
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct StDecoderInstanceRefV2 {
    pub handle: *mut core::ffi::c_void,
    pub vtable: *const StDecoderInstanceVTableV2,
    pub reserved0: u32,
    pub reserved1: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StConfigUpdateModeV2 {
    HotApply = 1,
    Recreate = 2,
    Reject = 3,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct StConfigUpdatePlanV2 {
    pub mode: StConfigUpdateModeV2,
    pub reason_utf8: StStr, // optional
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct StDecoderInstanceVTableV2 {
    pub open: extern "C" fn(handle: *mut c_void, args: StDecoderOpenArgsV2) -> StStatus,
    pub get_info: extern "C" fn(handle: *mut c_void, out: *mut StDecoderInfoV1) -> StStatus,
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
            out_plan: *mut StConfigUpdatePlanV2,
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
pub struct StDecoderOpenArgsV2 {
    pub path_utf8: StStr,
    pub ext_utf8: StStr,
    pub io_vtable: *const StIoVTableV1,
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
        StConfigUpdateModeV2::HotApply => {
            let block = build_param_block(new_config_json)?;
            rt.params.store(Arc::new(block));
            let gen = rt.config_gen.fetch_add(1, Ordering::AcqRel) + 1;
            Ok(UpdateOutcome::Applied { gen })
        }
        StConfigUpdateModeV2::Recreate => {
            let next_gen = rt.config_gen.load(Ordering::Acquire) + 1;
            rt.cmd_tx.send(InstanceCmd::Recreate {
                new_config_json: new_config_json.to_string(),
                target_gen: next_gen,
            })?;
            // Caller waits for actor ack/event and returns Recreated/Rejected.
            wait_recreate_result(next_gen)
        }
        StConfigUpdateModeV2::Reject => {
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
2. `IN_PROGRESS` Implement SDK codegen against the new ABI only.
Current: V2 SDK exists and now includes decoder extension-score export (`EXT_SCORE_RULES` -> ABI callbacks) plus decoder `open` IO bridge shim, but repository is not yet V2-only end-to-end.
3. `DONE` Introduce host runtime generation manager in `stellatune-plugins`.
Current: `PluginRuntimeService` includes native `load/reload/unload/list` management path, shared singleton access, and typed `create_*_instance` V2 execution APIs.
4. `IN_PROGRESS` Remove `PluginManager: Clone` usage in call-heavy paths.
Current: query control paths have moved to control-thread actor ownership for capability instances, but decode open/probe and library scan/metadata paths still depend on `PluginManager` snapshots/clones.
5. `IN_PROGRESS` Migrate `stellatune-audio` decode/output pipeline to instance-owner model.
Current: DSP and output sink execution use V2 instances; source/lyrics/output query is now owner-actor based with instance reuse; decoder selection is extension-score based and no longer uses legacy probe scoring; decoder open in audio now attempts V2 instance open first and falls back to legacy V1 decoder path.
6. `IN_PROGRESS` Migrate `stellatune-library` metadata scan/watch to instance-owner model.
Current: V2 instance wrappers are ready; library execution path wiring to V2 instances is not completed yet.
7. `NOT_STARTED` Delete all V1 ABI/types/symbols/adapters and old plugin runtime paths.
8. `NOT_STARTED` Remove temporary migration suffixes and keep only the new mainline API surface.
9. `IN_PROGRESS` Remove/avoid broad `unsafe impl Sync` on runtime containers.
Current: broad unsafe impls still exist on legacy containers and must be removed with V1 path deletion.
10. `IN_PROGRESS` Enforce modular file layout and readability constraints during migration.
Current: V2 code is split by runtime concerns; final cleanup still pending after legacy deletion.

### 14.1 Next Refactor Plan (From Current State)

1. Stage A: Decoder path migration in audio worker
   1. `DONE` Introduce a V2-native decoder selection strategy (replace legacy `probe_best_decoder*` dependency) using extension-score callbacks.
   2. `IN_PROGRESS` Migrate `open_engine_decoder` and related decode entrypoints to V2 instance factories.
   Current: audio decode open path is V2-first with fallback; full V1 removal waits for plugin migration.
   3. Keep built-in decoder fallback behavior unchanged while replacing plugin decoder selection/open.
2. Stage B: Library worker migration
   1. Migrate metadata/scan/watch decode capability checks and decoder open calls to V2 runtime.
   2. Remove library-side `PluginManager` snapshot/clone dependency.
3. Stage C: Runtime event path migration
   1. Move plugin host event ingress/egress from legacy manager helpers to V2 runtime-native path.
   2. Ensure reload/deactivate keeps actor-owned instance and event state consistent.
4. Stage D: Legacy deletion gate
   1. Delete V1 runtime execution paths once audio/library call sites are fully migrated.
   2. Remove broad legacy `unsafe impl Sync` containers together with V1 deletion.
   3. Rename transition-only `V2` surfaces where appropriate to become the mainline API.

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
3. `PENDING` Hot config update path works for at least one DSP and one output sink plugin.
4. `PENDING` Decode/output/library workers no longer depend on cloning whole plugin runtime state.
5. `PENDING` Per-instance serialization violations in host runtime are detected and surfaced as deterministic errors.
