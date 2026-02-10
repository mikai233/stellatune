# ASIO + Gapless Execution Plan

Status: Active  
Last Updated: 2026-02-10  
Owner: `stellatune-audio` / `stellatune-plugin-asio` / `stellatune-plugin-api`

## Scope

This document tracks the concrete execution steps for the currently observed issues:

1. Long wait when switching tracks with ASIO output.
2. "Gapless" currently means "faster switching", not sample-accurate gapless playback.
3. Local-track decoder path is built-in-first, reducing plugin decoder control for gapless tuning.
4. ASIO sample-rate strategy is not explicit/configurable in plugin UI for lessgap-focused playback.

## Engineering Policy

This project is in an early stage. For this plan we explicitly choose direct, breaking changes.

1. Do not add compatibility layers, dual paths, or legacy fallbacks only for migration.
2. Remove/replace old behavior directly when it conflicts with the target path.
3. Prioritize end-state architecture for lessgap playback over short-term compatibility.
4. If an ABI/API/data-model change is required for lessgap, update all call sites in one pass.

## Goals

1. Reduce ASIO track-switch wait to a near-instant stop/start path.
2. Keep output sink lifecycle stable across adjacent track switches.
3. Add real gapless metadata contract (encoder delay + end padding) and consume it in decode pipeline.
4. Make decoder selection strategy explicit/configurable for local tracks.
5. Add ASIO sample-rate policy config in plugin UI:
   - Resample to fixed OS-preferred/target sample rate.
   - Match output sample rate to each track sample rate.

## Work Board

- [x] P1. Fast track-switch path: avoid blocking flush on sink shutdown during switch.
- [x] P2. Keep ASIO sink/route stable to avoid unnecessary reopen on adjacent tracks.
- [x] P3. Gapless ABI upgrade: add delay/padding fields in decoder info (API + SDK + host).
- [x] P4. Decode pipeline gapless trim: apply start/end sample trimming.
- [ ] P5. Local decoder policy: add configurable priority (built-in first vs plugin first).
- [x] P6. ASIO sample-rate policy in plugin UI (fixed target vs per-track match).
- [ ] P7. End-to-end validation: latency metrics + audible transition verification.
  - [x] P7a. Runtime metrics instrumentation (switch latency / sink recreate / output sample-rate changes).
  - [ ] P7b. Manual audible transition verification and stress run.
- [ ] P8. ASIO lessgap stabilization sprint (progress jitter / crackle / switch wait).
  - [x] P8.1 Manual track switch path: do not tear down sink on `load_track(_ref)` when route identity is unchanged.
  - [x] P8.2 Position event contract: add track/session identity on public `Event::Position` (breaking ABI) and update UI consumer.
  - [x] P8.3 Gapless trim hot-path optimization: remove front-drain heavy behavior in decode loop.
  - [x] P8.4 Sink queue/watermark stabilization tuning: reduce `Playing<->Buffering` oscillation in ASIO mode.
  - [x] P8.5 ASIO sidecar underrun observability: explicit underrun counters/events for crackle diagnosis.
  - [x] P8.6 Transition fade policy split: use different fade ramp for track switch vs seek.

## Phase Details

### P1. Fast track-switch path

Status:
- Completed (2026-02-10)

Problem:
- Current output-sink shutdown path flushes pending ring-buffer audio before closing.
- ASIO plugin default `flush_timeout_ms=400`, making switch latency clearly visible.

Action:
- Introduce shutdown mode (`drain` vs `fast`).
- Use fast shutdown for the current engine switch path (no blocking flush wait).

Acceptance:
- Manual next/previous switching no longer waits on flush timeout.
- No regression in basic playback start/stop stability.

### P2. Stable sink lifecycle

Status:
- Completed (2026-02-10)

Problem:
- Reopen can still occur when negotiated sink spec changes frequently across tracks.

Action:
- Add explicit decode-session stop modes:
  - `TearDownSink`: hard stop and close sink worker.
  - `KeepSink`: stop decode session but keep sink worker alive.
- Use `KeepSink` on natural EOF transition path to preserve output sink between adjacent tracks.
- Keep `TearDownSink` for disruptive/manual/error/device-route changes.
- Reuse existing sink worker when plugin/type/target/spec identity still matches.

Acceptance:
- Same-device continuous playback shows no sink recreate for common adjacent tracks.

### P3/P4. True gapless support

Status:
- Completed (2026-02-10)

Problem:
- Decoder info only has sample-rate/channels/duration/seekable; no encoder delay/end padding.

Action:
- Extend decoder info schema with gapless fields:
  - `encoder_delay_frames`
  - `encoder_padding_frames`
- Propagate ABI and runtime handling through:
  - `stellatune-plugin-api` (ABI struct + API version bump)
  - `stellatune-plugin-sdk` (`DecoderInfo` -> FFI)
  - `stellatune-plugins` decoder wrapper defaults
  - built-in decoder and plugin decoders (`ncm` / `netease`)
- Apply decode-path trimming in `EngineDecoder`:
  - head trim (delay) once at stream start / seek-to-zero
  - tail holdback (padding) dropped at EOF
  - works for preload/open/seek flows because trimming is centralized in `next_block`.

Acceptance:
- Encoder-delay formats (e.g. MP3/AAC with metadata) transition without injected silence at boundaries.

### P5. Decoder selection policy

Problem:
- Local tracks are built-in-first by design now.

Action:
- Add configurable local decoder preference.
- Keep deterministic fallback behavior.

Acceptance:
- Local tracks can run with plugin-first policy without breaking fallback.

### P6. ASIO sample-rate policy in plugin UI

Status:
- Completed (2026-02-10)

Problem:
- Current ASIO behavior does not expose an explicit sample-rate policy in plugin UI.
- Lessgap playback tuning needs a controllable strategy:
  1) fixed target rate (resample),
  2) per-track rate match.

Action:
- Add ASIO config fields and schema for sample-rate policy.
- Expose policy in plugin UI panel (host-rendered plugin settings).
- Support two modes:
  - `fixed_target`: resample to configured target (typically OS-preferred rate).
  - `match_track`: negotiate/reopen to match each track sample rate.
- Wire runtime apply behavior to existing output-sink update flow.

Acceptance:
- User can see and change this policy in plugin UI.
- Playback path clearly uses selected mode (verified by logs/runtime events).
- `fixed_target` mode keeps stable output rate across tracks.
- `match_track` mode follows track sample rate per item.
- No compatibility shim; direct breaking update of config/schema/runtime paths.

### P7. Validation

Status:
- In progress (2026-02-10)
- Metrics instrumentation completed (2026-02-10)
- Manual audible verification pending

Metrics:
- Track switch command-to-buffering-complete latency.
- Output sink recreate count per 100 track transitions.
- Preload hit ratio.
- Sample-rate policy behavior:
  - fixed mode: output rate change count should be near zero across mixed-rate queue.
  - match mode: output rate should follow track rate transitions.

Manual checks:
- Next/previous stress test.
- Mixed sample-rate queue.
- Long playback run with ASIO sidecar.

### P8. ASIO lessgap stabilization sprint

Status:
- In progress (2026-02-10)
- P8.1 completed (2026-02-10)
- P8.2 completed (2026-02-10)
- P8.3 completed (2026-02-10)
- P8.4 first tuning pass completed (2026-02-10)
- P8.5 completed (2026-02-10)
- P8.4 second tuning pass completed (2026-02-10): ASIO manual switch no longer applies fade-in mute that can eat leading audio.
- P8.4 third tuning pass completed (2026-02-10): unified startup transition policy (play/switch starts at unity gain; no hard-mute on incoming session).
- P8.6 completed (2026-02-10): split disrupt fade ramp by action kind (`track_switch` vs `seek`), with longer seek fade to reduce abrupt gain jumps.

Observed issues (latest):
- In ASIO mode, progress movement is discontinuous (jitter / occasional rollback).
- Crackle/dropout artifacts appear during playback (suspected to be related to recent lessgap decode path changes).
- Track switch still waits around ~2s in real usage even with resample mode.

Execution order:
1. P8.1 first to remove avoidable sink reopen during manual switch flow.
2. P8.3 + P8.4 to stabilize decode/sink real-time behavior (primary crackle risk area).
3. P8.2 to complete external event contract hardening for position ownership.
4. P8.5 to improve diagnostics and close the loop with measurable underrun evidence.

Acceptance:
- Manual next/previous switch latency no longer includes sink reopen path in steady route/spec cases.
- No audible crackle/pop in 30min ASIO mixed-sample-rate run (fixed_target mode).
- Progress bar no longer shows non-seek rollback/jitter caused by stale position ownership.
- Runtime logs include explicit underrun counters for ASIO sidecar path.

## Checklist Alignment (7-item, 2026-02-10)

This section maps the latest implementation status to the 7-item improvement checklist used in review.

1. Manual switch path does not tear down ASIO sink (`P0`)
- Status: Done.
- Notes: `load_track` / `load_track_ref` now stop decode session with `KeepSink` instead of `TearDownSink`.
- Refs: `crates/stellatune-audio/src/engine/control/commands/playback.rs:18`, `crates/stellatune-audio/src/engine/control/commands/playback.rs:46`, `crates/stellatune-audio/src/engine/control.rs:188`.

2. Position event carries track/session identity (`P0`, breaking ABI)
- Status: Done.
- Notes: `Event::Position` now includes `{ ms, path, session_id }` and all control-path emits are unified through one helper. Flutter consumer now filters by current track path and active session id, preventing stale-position rollback/jitter after switch/seek.
- Refs: `crates/stellatune-core/src/playback.rs:236`, `crates/stellatune-audio/src/engine/control/commands/playback.rs:72`, `crates/stellatune-audio/src/engine/control/internal/errors.rs:95`, `apps/stellatune/lib/player/playback_controller.dart:1029`.

3. Gapless trim hot-path performance refactor (`P0`)
- Status: Done.
- Notes: Gapless trim state switched to deque-based buffering to avoid front-drain-heavy behavior.
- Refs: `crates/stellatune-audio/src/engine/decode/decoder.rs:192`, `crates/stellatune-audio/src/engine/decode/decoder.rs:196`.

4. ASIO sink queue and watermark retune (`P0`)
- Status: Partial.
- Notes: Queue depth/watermark tuning is applied; explicit multi-tick hysteresis for state switching is not yet implemented.
- Refs: `crates/stellatune-audio/src/engine/session.rs:29`, `crates/stellatune-audio/src/engine/control.rs:70`, `crates/stellatune-audio/src/engine/control.rs:71`, `crates/stellatune-audio/src/engine/control/output_sink.rs:38`.

5. Sidecar underrun observability (`P1`)
- Status: Partial.
- Notes: Sidecar underrun counters and periodic summary logs are implemented. Plugin runtime event reporting for underrun is not wired yet.
- Refs: `crates/stellatune-asio-host/src/main.rs:410`, `crates/stellatune-asio-host/src/main.rs:441`, `crates/stellatune-asio-host/src/main.rs:478`.

6. Real-time resample quality preset (`P1`)
- Status: Partial.
- Notes: ASIO sample-rate mode (`fixed_target` / `match_track`) is done. Resampler quality level is still fixed high-quality (`SINC_LEN=256`) and not yet exposed as runtime preset.
- Refs: `crates/stellatune-plugin-asio/src/lib.rs:33`, `crates/stellatune-plugin-asio/src/lib.rs:81`, `crates/stellatune-audio/src/engine/config.rs:31`.

7. Merge switch flow into one command (`P1`)
- Status: Done.
- Notes: Added public command `SwitchTrackRef { track, lazy }` (breaking ABI) and wired it end-to-end (`core`/`plugin protocol`/`backend`/`FFI`/Flutter bridge). Queue-driven切歌 now uses single command (`lazy=false`) instead of `LoadTrackRef + Play`. Legacy switch commands `LoadTrack` / `LoadTrackRef` were removed from command/protocol/FFI API.
- Refs: `crates/stellatune-core/src/playback.rs:80`.

## Change Log

- 2026-02-10: Created plan.
- 2026-02-10: P1 completed. Added shutdown mode (`drain`) and switched engine sink-worker shutdown to fast path (`drain=false`).
- 2026-02-10: P2 completed. Added session stop modes (`TearDownSink` / `KeepSink`) and kept sink worker across EOF transition path to reduce adjacent-track reopen.
- 2026-02-10: P3/P4 completed. Added gapless delay/padding fields to decoder ABI (`STELLATUNE_PLUGIN_API_VERSION=6`) and implemented head/tail trim in `EngineDecoder` decode path.
- 2026-02-10: Added explicit policy: no compatibility code; breaking changes are allowed and expected for lessgap target.
- 2026-02-10: Added ASIO sample-rate policy requirement in plugin UI (`fixed_target` vs `match_track`) for lessgap tuning.
- 2026-02-10: P6 completed. `stellatune-plugin-asio` now exposes `sample_rate_mode` + `fixed_target_sample_rate` in config schema and applies mode-specific negotiate behavior.
- 2026-02-10: P7a completed. Added runtime metrics hooks for track-switch latency, output sink recreate count, and output sample-rate change count (aggregated logs for validation runs).
- 2026-02-10: Added P8 stabilization sprint plan for current ASIO issues (progress jitter / crackle / residual switch wait).
- 2026-02-10: P8.1 completed. Manual `load_track`/`load_track_ref` no longer force sink teardown; switch path now keeps sink lifecycle stable.
- 2026-02-10: P8.3 completed. Gapless trim state switched to deque-based queueing to avoid front-drain heavy behavior in hot decode loop.
- 2026-02-10: P8.4 first pass completed. Increased sink message queue capacity and raised minimum sink watermarks to reduce ASIO buffering oscillation/crackle risk.
- 2026-02-10: P8.5 completed. `stellatune-asio-host` now tracks underrun callback/sample counters and emits throttled per-second underrun summary logs from a dedicated reporter thread.
- 2026-02-10: P8.4 second pass completed. For output-sink (ASIO) route, removed start-of-track fade-in mute on manual switch path to prevent leading audio loss.
- 2026-02-10: P8.4 third pass completed. Unified play/switch transition logic to always start incoming session at unity gain; fade-out remains only on outgoing/disrupt path.
- 2026-02-10: P8.6 completed. Split transition fade durations by action type: `track_switch` uses shorter ramp, `seek` uses longer ramp. Fade wait timeout now derives from selected ramp + extra guard window.
- 2026-02-10: P8.6 follow-up. Fixed seek fade trigger reliability: seek disrupt fade now also applies in `Buffering` state (when playback is requested), with wait only when pending audio exists to avoid no-op wait.
- 2026-02-10: P8.6 follow-up #2. Fixed occasional post-seek/switch silence by forcing unity gain reset on pending-session start and adding ASIO buffering fallback resume (low-watermark + timeout) when transition target remains faded out.
- 2026-02-10: P8.6 follow-up #3. Fixed occasional hard-cut on manual track switch: disrupt fade now also applies when state is `Buffering` (not only `Playing`) as long as playback is active and pending audio exists.
- 2026-02-10: P8.6 follow-up #4. Mitigated start-of-track crackle/pop by adding a short session-start entry ramp (now tuned to 5ms) on new session play instead of hard jump to unity gain (generic audio-layer policy, not ASIO-specific).
- 2026-02-10: P8.6 follow-up #5. Transition gain curve switched from linear gain-domain stepping to non-linear equal-power interpolation (power-domain lerp + sqrt) in `MasterGainProcessor` for smoother seek/switch/session fades.
- 2026-02-10: P8.6 follow-up #6. Added lessgap-specific head-trim de-click ramp in `GaplessTrimState` (2ms equal-power over first effective frames after head trim/seek-to-zero) to reduce start-of-track crackle likely caused by abrupt post-trim sample boundary.
- 2026-02-10: P8.6 follow-up #7. Fixed first-random-seek fade inconsistency: seek now always applies disrupt fade when playback is active (not gated by pending queue estimate), while fade wait policy remains conditional to avoid unnecessary buffering delays.
- 2026-02-10: P8.6 follow-up #8 (reverted). The `smoothstep` transition envelope experiment and always-on decoder entry de-click were rolled back after verification; transition/de-click behavior returned to the previous baseline (`sqrt(t)` based entry shaping and head-trim-triggered decoder entry ramp).
- 2026-02-10: P8.6 follow-up #9. Added output-sink disrupt reset path to reduce boundary residual noise: on seek/switch, worker now drops queued pending sink samples; on manual track switch with pending queue, sink worker performs in-place sink recreate (ring reset effect for ASIO sidecar) before next track output binding. Reset path is independent from `seek_track_fade` toggle.
- 2026-02-10: P8.6 follow-up #10 (closed, reverted). Temporary lessgap-trim bypass experiment was rolled back after A/B verification; current decoder path remains the normal lessgap-trim implementation, and the residual boundary noise is likely not primarily from this path.
- 2026-02-10: P8.6 follow-up #11. Added generic output-boundary diagnostics for manual track switch (begin/commit logs with buffered/pending samples) and fixed potential stale-scratch carryover in `GatedConsumer`: when output gate closes, staged samples are explicitly dropped before next enable, reducing occasional boundary noise on non-ASIO + fade-off path.
- 2026-02-10: P8.6 follow-up #12. Added startup anti-thrash guards on shared output path: reset reused output-pipeline runtime state (`output_enabled=false`, `buffered_samples=0`) before preparing next session, and require consecutive buffering-ready ticks before reopening output gate (`BUFFER_RESUME_STABLE_TICKS=2`) to avoid open-then-immediate-underrun oscillation on track switch.
- 2026-02-10: P8.6 follow-up #13. ASIO startup crackle mitigation for device-switch path: plugin now delays sidecar `Start` until shared-ring prefill threshold (`start_prefill_ms`, default 30ms) is reached. In `fixed_target` mode, `fixed_target_sample_rate` is enforced exactly when configured; when null, plugin uses device/OS default sample rate by design.
- 2026-02-10: P8.6 follow-up #14. ASIO `preferred_chunk_frames` supports dynamic auto mode (`0`): chunk size is now derived from output sample rate (~2.7ms target, rounded to power-of-two; 48k->128, 96k->256, 192k->512), while positive values keep manual override.
- 2026-02-10: P8.6 follow-up #15. Reduced switch-log noise and added merged timing summary: manual switch `begin/committed` logs moved to `trace`, and one `debug` summary is emitted when playback resumes (`manual track switch timing summary`) with phase breakdown (`fade_wait`, `stop_after_fade`, `pre_play_idle`, `session_prepare`, `buffering_wait`, `play_to_audible`). Wall-clock `total` is downgraded to `trace` (`manual track switch wall timing`) to avoid user-idle misinterpretation.
- 2026-02-10: P8.6 follow-up #16. Reduced ASIO underrun log spam during pause/idle: sidecar underrun reporter now suppresses interval logs when no audio samples were delivered in that interval (`delta_delivered_samples=0`), while keeping underrun stats visible during active playback.
- 2026-02-10: P8.6 follow-up #17. Added rapid switch command coalescing in control loop: adjacent `LoadTrack/LoadTrackRef/Play` bursts are merged into final-effective commands (latest track + at most one play), with non-switch commands preserved in order. This removes cumulative per-track switch overhead during fast next/previous spam.
- 2026-02-10: P8.6 follow-up #18. Disabled disrupt-time output sink instance recreate on track switch (keep sink, drop pending queue only) to avoid serial sink reopen cost amplification in ASIO rapid-switch scenarios.
- 2026-02-10: P8.2 follow-up. Fixed seek progress bounce/rollback: added internal seek position guard in control (`target/origin/requested_at`, timeout+tolerance acceptance), reset guard on load/stop/error/eof, and removed decode-thread direct `Event::Position` emit so all public position events are filtered/emitted from control only.
- 2026-02-10: P8.4 follow-up #19. Added ASIO plugin `latency_profile` UI config (`aggressive` / `balanced` / `conservative`) to let users choose buffering strategy. Default is now `conservative`; auto chunk/prefill use profile-driven scaling when `preferred_chunk_frames=0` and `start_prefill_ms=0`.
- 2026-02-10: P8.4 follow-up #20. Mitigated minimize/background underrun risk by enabling OS realtime scheduling hint (Windows MMCSS `Pro Audio`) on generic audio-critical worker threads (`control`, `decode`, `output-sink`) in the audio engine process. This is backend-agnostic and also helps plugin-sink/ASIO path when UI is minimized.
- 2026-02-10: P8.4 follow-up #21. ASIO `latency_profile` default changed from `conservative` to `balanced` (schema + runtime default + README sample) to provide lower startup latency while preserving user-selectable conservative mode.
- 2026-02-10: P8.2 completed. Public playback event ABI changed: `Event::Position` now carries `ms + path + session_id`; Rust control emits are unified via `emit_position_event`, and Flutter position consumer applies track/session ownership filtering to avoid stale progress rollback.
- 2026-02-10: Checklist item #7 completed. Added single switch command `SwitchTrackRef { track, lazy }` across command ABI/protocol/FFI, updated control-loop burst coalescing to absorb trailing `Play` into `SwitchTrackRef(lazy=false)`, and switched Flutter queue playback path to call this command directly.
- 2026-02-10: Breaking cleanup for checklist item #7: removed legacy switch commands `LoadTrack` / `LoadTrackRef` from `stellatune-core::Command`, control protocol enums, plugin control JSON contract, backend/FFI endpoints, and Flutter bridge API.
- 2026-02-10: Unified switch-transaction follow-up: `SwitchTrackRef(lazy=false)` no longer performs synchronous `on_play` session start; playback start is armed and executed by tick path with a short backend-agnostic settle window (`SWITCH_SESSION_START_SETTLE_MS`) to absorb rapid switch bursts before expensive session init. Flutter local path no longer optimistically rewrites `currentPath` on switch request, reducing UI/audio mismatch during burst switching.
