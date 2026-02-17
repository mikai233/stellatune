# Music Backend Trait Pipeline Breaking Refactor Plan

Status: `Active`  
Last Updated: `2026-02-16`  
Owner: `stellatune-audio` / `stellatune-audio-core` / adapters / backend-api / ffi / Flutter host  
Scope: **Breaking change only**. No compatibility facade after cutover.

## 1. Decision Snapshot

These decisions are fixed unless explicitly changed:

1. `stellatune-audio` is the final engine to replace the legacy `stellatune-audio-legacy` engine.
2. `MasterGain` is immediate-apply (no volume ramp in control path for now).
3. `Stop` clears `current_track` in snapshot/event semantics.
4. Documentation follows current implementation contracts first; pseudocode must not diverge from code contracts.

## 2. Why This Refactor Exists

The legacy engine mixes playback orchestration, plugin runtime details, and output routing logic too tightly, making maintenance and extension expensive.

The v2 direction keeps a strict pipeline contract:

1. Stage-oriented processing (`Source -> Decoder -> Transform* -> Sink`).
2. Plugin/builtin implementation parity at orchestration layer.
3. Runtime assembly chooses concrete adapters; core contracts stay plugin-agnostic.

## 3. Current Reality (Implemented)

### 3.1 Core Contracts

`stellatune-audio-core` currently provides:

1. `SourceStage`, `DecoderStage`, `TransformStage`, `SinkStage`.
2. `PipelineContext`, `PipelineError`, stream/block primitives.
3. `StageStatus` is `Ok | Eof | Fatal` (no `Backpressure` enum variant).

### 3.2 v2 Runtime

`stellatune-audio` currently provides:

1. `PipelineRunner` + decode loop worker.
2. Sink worker with non-blocking audio ring write path.
3. Transform assembly with:
   - `GaplessTrimStage`
   - optional `MixerStage` (channel remap/upmix/downmix + `LfeMode`)
   - optional `ResamplerStage` (target sample-rate transform)
   - explicit DSP pre/post chain insertion points (`pre_mix` / `post_mix`)
   - `TransitionGainStage` (seek/pause/stop/switch fades)
   - `MasterGainStage` (immediate apply)
4. Fade behavior wired into command handlers (`seek/pause/stop/open/play`).
5. Near-EOF fade hint channel:
   - decoder optional `estimated_remaining_frames()`
   - runner computes playable hint (gapless-tail deduction + decoder/output sample-rate domain alignment) and feeds `available_frames_hint`.
6. Runner drain path now attempts bounded transform-tail extraction before sink flush, to avoid dropping flush-generated transform output blocks.
7. Runtime control surface additions:
   - typed per-command control paths exist for `set_volume` / `set_lfe_mode` / `set_resample_quality` / `set_dsp_chain` (`EngineHandle -> control handler -> decode loop`), with policy persistence where applicable.
   - generic stage control path is exposed as `apply_stage_control(stage_key, Box<dyn Any + Send>)` on control/decode-loop layers; it is precise stage delivery (no control-type probing loop over transform vec).
   - stage control persistence is supported: controls applied via `apply_stage_control` are replayed on runner rebuild paths (`open`, `prewarm promote`, `reconfigure`, `apply_pipeline_plan`, sink-recovery rebuild).
   - transform stage key validation is fail-fast at runner construction: empty key or duplicate key returns `PipelineError::StageFailure`.
   - `set_dsp_chain` remains typed (`DspChainSpec`) and forwards to `PipelineRuntime::apply_dsp_chain`; raw stage-control packet pass-through API is not exposed on v2 public control surface.

### 3.3 Semantics to Preserve

1. Sync pipeline execution only (no async stage contract).
2. Decode thread must not block on sink writes.
3. Sink disconnect recovery remains deterministic and bounded.

## 4. Contract Baseline (Do Not Drift)

### 4.1 Stage Status and Runner Result

Current baseline:

1. `StageStatus`: `Ok | Eof | Fatal`
2. `StepResult`: `Idle | Produced { frames } | Eof`

Backpressure is currently represented by runner-local pending block + `StepResult::Idle`, not by a dedicated status/result variant.

### 4.2 Stop/Eof/Error Snapshot Semantics

Target semantics for control snapshot/event handling:

1. `Stop`: clear `current_track`, set position to `0`, state `Stopped`.
2. `Eof` (no next track): clear `current_track`, position `0`, state `Stopped`.
3. Fatal teardown path: clear `current_track`, state `Stopped`.
4. Optional UI convenience field can be added later (`last_track`) without changing `current_track` semantics.

## 5. Capability Gap to Reach Replacement

### P0 (Must Have Before Cutover)

1. Mixer stage parity
   - Channel remap/upmix/downmix and LFE mode behavior equivalent to legacy.
2. Resampler stage parity
   - Quality policy and output sample-rate targeting.
3. DSP transform chain parity
   - Pre/post chain insertion and runtime control synchronization.
4. Output routing parity
   - Device sink and output-sink route with multi-sink fan-out behavior.
5. Control API parity against legacy public surface
   - At minimum: volume, output device/options, output sink route, preload, refresh devices.
6. Snapshot/event semantic hardening
   - Apply and test the stop/eof/error `current_track` clearing policy.

Current P0 status:

1. Implemented in v2 runtime: mixer/resampler stages, DSP pre/post insertion, transition/gapless/master builtins, and stage-keyed runtime control baseline.
2. Remaining P0 focus before cutover: output routing parity (device + plugin sink route details), legacy public control surface parity gaps (device/options/preload/refresh devices), and backend/ffi full path replacement.

### P1 (Should Have Near Cutover)

1. Real adapter implementation of `estimated_remaining_frames()` (builtin/plugin decoders).
2. Decode-loop recovery integration tests (disconnect/backoff/rebuild lifecycle).
3. Hot-path allocation reduction and runtime timing metrics for v2.

### P2 (Post-Cutover Improvements)

1. Optional `last_track` snapshot field for UI convenience.
2. Optional master gain smoothing policy (if user experience requires it).
3. Further core/adapter profile negotiation improvements.

## 6. Execution Plan

### Phase R0: Contract and Doc Alignment (In Progress)

1. Remove stale pseudocode and stale status variants from this document.
2. Keep plan text aligned with implemented contracts and naming.
3. Freeze baseline semantics in this doc.

Exit criteria:

1. No plan item references unavailable status/result variants.
2. Team can derive test assertions directly from this document.

### Phase R1: Transform and Audio Path Parity

1. Keep `MixerStage` / `ResamplerStage` / DSP pre/post chain behavior aligned with legacy edge cases.
2. Continue parity hardening with real adapter/backend scenarios (not only isolated unit tests).
3. Maintain deterministic control+rebuild semantics for transform graph updates.

Exit criteria:

1. Manual parity check for representative stereo/mono/LFE/sample-rate cases.
2. Automated regression coverage for transform chain order and output invariants.

### Phase R2: Control/API Parity

1. Maintain per-command APIs for common operations (`set_volume` etc.) while supporting generic stage-key control for extensibility.
2. Keep `MasterGain` command path immediate-apply and rebuild-safe.
3. Fill remaining output routing and preload parity APIs.

Exit criteria:

1. Backend/FFI can call only v2 APIs for current product feature set.
2. No required legacy-only control operation remains.

### Phase R3: Backend Cutover

1. Switch backend runtime engine construction from legacy engine to v2 engine.
2. Keep a short-lived internal rollback toggle only during validation window.
3. Remove rollback path after validation signoff.

Exit criteria:

1. `backend-api` default engine is v2.
2. CI/manual matrix passes on supported platforms.

### Phase R4: Legacy Deletion

1. Delete obsolete legacy decode/output orchestration modules.
2. Remove legacy engine wiring from backend/ffi layers.
3. Remove migration-only notes and dead code.

Exit criteria:

1. Single authoritative engine path remains.
2. Workspace builds/tests without legacy engine references.

## 7. Validation Matrix

### Build/Test

1. `cargo check -p stellatune-audio-core -p stellatune-audio -p stellatune-audio-builtin-adapters -p stellatune-audio-plugin-adapters`
2. `cargo test -p stellatune-audio-core -p stellatune-audio`

### Behavior

1. Manual: play/pause/seek/stop/switch with expected state transitions.
2. Manual: near-EOF seek/pause/stop/switch fade behavior.
3. Manual: sink disconnect and recovery behavior.
4. Manual: output route parity (device + plugin sink route where applicable).
5. Manual: preload parity with playback resolution chain.

### Integration

1. backend-api uses v2 runtime path.
2. ffi and Flutter host command paths validated against v2 feature parity list.

## 8. Risks and Mitigations

1. Risk: runtime regressions from incomplete parity.
   Mitigation: phase gates by API and behavior matrix, not by code completion percentage.
2. Risk: real-time jitter from hot-path sync/control overhead.
   Mitigation: add timing metrics around `runner.step` and sink control sync path, optimize by dirty-bit control propagation.
3. Risk: ambiguous state semantics causing UI/backend drift.
   Mitigation: keep explicit snapshot contract in this document and enforce with tests.

## 9. Progress Log

Use this section for milestone-level updates only.

- 2026-02-16: Plan updated to align with v2 implementation contracts and remove stale status/result pseudocode.
- 2026-02-16: Decision freeze recorded: v2 final replacement, immediate master gain, stop clears current track.
- 2026-02-16: Gapless, transition gain, master gain stages integrated in v2 transform chain.
- 2026-02-16: Near-EOF fade hint channel integrated (`estimated_remaining_frames -> playable_remaining_frames_hint -> available_frames_hint`).
- 2026-02-16: Command-flow integration tests added for seek/pause/stop/switch near-EOF fade behavior.
- 2026-02-16: Master gain control command chain wired (`EngineHandle.set_volume -> control handler -> decode loop -> PipelineContext/MasterGainStage`) with immediate sync attempt and level persistence across context rebuilds.
- 2026-02-16: Snapshot semantics hardened: `stop/eof/error` now clear `current_track`; EOF additionally resets position to `0`.
- 2026-02-16: Control-handler integration tests added for `stop/eof/error` track-clearing semantics and `set_volume` command forwarding.
- 2026-02-16: v2 audio path now supports optional `MixerStage` / `ResamplerStage` and explicit DSP pre/post transform chain assembly ordering.
- 2026-02-16: `playable_remaining_frames_hint` now scales from decoder frame domain to output frame domain (after gapless tail deduction), and near-EOF fade command tests cover the resampler/gapless cases.
- 2026-02-16: Runner `drain()` now includes bounded transform-tail extraction before sink drain; regression test added to ensure flush-generated transform tail reaches sink.
- 2026-02-16: Runtime control APIs added for `set_lfe_mode` / `set_resample_quality` / `set_dsp_chain`; decode-loop now applies mixer/resampler policy overrides when assembling/rebuilding runners.
- 2026-02-16: `set_dsp_chain` upgraded to typed `DspChainSpec` and forwarded through `PipelineRuntime::apply_dsp_chain`; adapter-side v1 payload parsers exist in builtin/plugin adapter crates.
- 2026-02-16: Removed v2 raw stage-control compatibility path from public control/decode-loop APIs (`send_stage_control` pass-through); kept only typed control entry points.
- 2026-02-16: Kept per-command API surface (`set_volume` / `set_dsp_chain` / `set_lfe_mode` / `set_resample_quality`) while adding generic stage-keyed control (`apply_stage_control`) for builtin/external transform unification.
- 2026-02-16: Runner transform control dispatch changed to precise `stage_key -> transform index` routing; removed control-type probing/vec traversal behavior.
- 2026-02-16: Added persisted stage-control replay across runner lifecycle transitions (`open`, `prewarm promote`, `reconfigure`, `apply_pipeline_plan`, sink recovery rebuild).
- 2026-02-16: Enforced strict transform stage-key validation (empty/duplicate keys fail fast) and added regression tests for duplicate key rejection and control replay behavior.
