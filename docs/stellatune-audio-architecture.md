# `stellatune-audio` Internal Architecture

This document describes the implemented playback boundaries. The complete
hard-switch design and acceptance criteria are in
[`player-core-refactor.md`](player-core-refactor.md).

## 1. Ownership and control

`playback::runtime::PlaybackRuntime` owns a Lattice `PlaybackActor` lifecycle.
The actor owns its active session and therefore the `SinkWorker` lifecycle.
Cloneable `playback::control::PlaybackController` values hold an
`ActorHandle<PlaybackActor>` and are typed command endpoints; dropping a
controller does not stop the runtime.

```text
Flutter / TUI
    -> PlayerService (queue order, navigation intent, resolution, persistence)
    -> PlaybackController (PlaybackItem commands)
    -> PlaybackActor (state, generation, epoch, transition, recovery)
    -> bounded PCM channel
    -> SinkWorker (final gain, markers, device clock)
    -> SinkStage
```

The bounded Lattice mailbox carries typed control requests, pump ticks, and
preparation/recovery completion messages only. `PlaybackState` is the Lattice
behavior; `PlaybackSession` contains generation, current/next tracks, seek and
transition data, but no duplicate state field. The actor uses a one-worker
`DedicatedThreadPool`, a turn budget of 16 by default, and a 2 ms interval that
may drop a saturated tick and resume automatically. Encoded bytes and PCM never
pass through FFI, JSON-RPC, or an actor mailbox.

Controller asks have operation-specific deadlines: snapshot 2 seconds,
ordinary controls and advancement 5 seconds, output rebuild 10 seconds, and switch/set-next
preparation 30 seconds. A deadline becomes `CommandTimeout`; invalid behavior
admission becomes `InvalidState`; a stopped lifecycle becomes `Closed`.

## 2. Source and planning path

There is one source-of-truth path:

```text
TrackId
    -> Source/Track Catalog
    -> SourceResolver
    -> validated ResolvedSourceSpec
    -> bound SourceFactory
    -> EncodedSource
    -> DecoderStage
```

`PipelinePlanner` receives a typed `PlaybackItem` and an immutable registry
snapshot. It selects ordered decoder candidates, stable transform placement,
the sink factory, and typed policies. It does not parse paths, URLs, provider
keys, or JSON.

Source open and decoder preparation run in Lattice deferred work, with blocking
decoder/configuration work isolated by Tokio's blocking pool. Request-backed
switch/queue work uses `defer_reply`; recovery uses `pipe_to_self`. Every
completion carries a preparation ID and generation. Replacing a session or
stopping cancels all of its work. The successor slot owns a separate preparation
token, so replacing it does not cancel recovery and claiming it does not cancel
its open task. Recovery likewise retains independently prepared successor work. A
preparation deadline also cancels its source token; current-track timeout fails
the session, next-track timeout preserves the current track, and recovery keeps
the bounded retry policy. Decoder fallback is limited to preparation and
requires a reopenable source. The decoder never opens a file or HTTP locator;
those responsibilities stay in source factories.

## 3. Audio data plane

The ordinary path is:

```text
EncodedSource -> Decoder -> gapless trim -> pre-mix transforms
              -> mix-format normalizer -> track gain -> Mixer
              -> shared post-mix transforms -> bounded PCM channel
              -> SinkWorker final gain -> SinkStage
```

`EncodedSource::read` and decoder/DSP calls are synchronous and bounded. HTTP
uses a bounded feeder; an empty feeder returns `WouldBlock`, which becomes a
typed pending decode turn rather than a network wait in the pump.

Transforms have explicit `PreMix` and `PostMix` placement. Buffered transforms
are drained at EOF before promotion. A core-owned normalizer converts the next
track to the current mix sample rate and channel layout before overlap, trims
resampler startup delay, and drains its filter tail to the exact target frame
count.

`PcmFormat` carries a strong `ChannelLayout`, not an independent channel count
and optional mask. The layout is a non-empty positioned-speaker set in the
canonical WAVEFORMATEXTENSIBLE interleaving order, and the channel count is
always derived from it. The supported domain is mono through 7.1.4 speaker
audio; discrete/custom channel orders and Ambisonics are rejected.

The normalizer composes a matrix-based `ChannelMixer` with the sample-rate
converter. Exact positions pass through, downmixing folds center, surround,
wide, and height positions toward the nearest available speaker, and expansion
leaves absent target positions silent. LFE is copied only to LFE and is never
synthesized. Matrix rows are normalized when their absolute coefficient sum
would exceed unity. Windows output obtains the precise endpoint channel mask;
backends without position metadata may infer only mono or stereo and reject
unknown multichannel layouts.

## 4. Transitions and position

- Gapless prewarms next, trims encoder delay/padding, reuses compatible output,
  and applies no transition gain.
- FadeOutIn drives one track pipeline at a time with frame-based envelopes.
- Crossfade drives current and next simultaneously, applies independent gains
  before the mixer, and runs post-mix DSP once.

If next fails after overlap starts, the failure remains associated with the
next `PlaybackItemId`; the current pipeline keeps its pending PCM and ramps from
its instantaneous crossfade gain back to unity.

Item-boundary markers travel in order with PCM. When an output is reused,
`TrackChanged` follows consumption of its marker. Initial activation and an
activation that creates a new output announce the new item directly. Command
acceptance is therefore not a promise that the device has already emitted sound. Public position and recovery
checkpoints use sink-consumed frames, never the decoder cursor or queued-ring
frontier.

## 5. Backpressure, seek, and recovery

The actor processes at most one decode block per pump turn. The PCM channel and
HTTP encoded feeder are bounded. The sink worker uses a separate bounded
high-priority control channel so pause, seek/discard, gain, and shutdown can
preempt a partial write that returns `WouldBlock`.

Seek increments the PCM epoch, discards old queued audio, resets transforms and
normalization state, continues pending decoder seek over bounded actor turns,
and applies a frame-based short fade at the actual seek result.

Recoverable decoder I/O and sink disconnects enter `Recovering`. The actor
captures the consumed checkpoint, performs a bounded source reopen/decoder
prepare/seek outside the actor, rebuilds the sink, and resumes only the prior
item. Retry count and backoff are typed playback policies.

`PlaybackRuntime::shutdown` subscribes to actor termination before requesting
`StopReason::Requested`, then waits for the stopping hook. That hook cancels
preparation, closes outstanding domain replies, resets decoder/DSP state and
shuts down the `SinkWorker`. `Drop` is best-effort only; composition roots use
explicit shutdown when deterministic device release matters. Actor panic or
lifecycle failure is terminal and must be recovered by creating a new runtime.

## 6. Application persistence

`player_service::catalog::PlayerCatalog` belongs to
`stellatune-backend-api`, not the audio crates. It stores typed source, track,
and playback-item identities plus queue/current and media-time position under
one strict schema fingerprint. `player_service::service::PlayerService`
orchestrates startup restore; `player_service::resolver` re-resolves local paths
and provider sources and rebuilds every runtime stage.
Temporary URLs, HTTP headers, stage instances, generations, and epochs are not
persisted. Unknown or partial player schemas fail without migration or repair.
State validation and current/position writes share one SQLite transaction. The
persisted queue retains the complete user queue and catalog deletion uses
tombstones, so stable IDs are never reassigned. Local identity registration,
queue validation/insertion, and path projection use batched SQL statements.
Projection uses captured track IDs, so concurrent queue edits cannot erase
the source identity of an earlier snapshot. Flutter indexes occurrence IDs
when projecting traversal order instead of repeatedly searching the list.

Flutter reads native queue and playback snapshots to project restored state into the UI.
It does not keep a second Hive track/position resume record or replay a second
switch/seek transaction at startup.

## 7. Public failures

Controller commands return `PlaybackControlError`. Playback-task failures use
`PlaybackFailure`, with typed `FailureStage`, retry disposition, stable code,
optional stage/item identity, and generation. FFI may turn its message into a
UI error, but the audio/application boundary no longer exposes a string stage
field as the error category.

## 8. Crate boundaries

```text
stellatune-audio-core <- stellatune-audio
stellatune-audio-core <- stellatune-audio-builtin-adapters

stellatune-library --------X stellatune-audio*
audio-builtin-adapters ----X stellatune-audio runtime
```

ASIO remains an optional external sidecar. TypeScript plugins stay on the
control plane and return declarative source resolution results; they never
transport encoded media or PCM.

`stellatune-audio-core` does not flatten its public API at the crate root.
Consumers use the owning module as the canonical and only path, for example
`format::PcmFormat`, `playback::PlaybackItem`, `source::SourceFactory`, and
`decoder::DecoderStage`. This keeps data models, stage SPIs, shared errors, and
identities visible in dependency declarations instead of hiding them behind a
crate-wide facade.

## 9. Reading order

1. `crates/stellatune-audio-core/src/format.rs`
2. `crates/stellatune-audio-core/src/playback.rs`
3. `crates/stellatune-audio-core/src/source.rs`
4. `crates/stellatune-audio-core/src/stage.rs`
5. `crates/stellatune-audio-core/src/error.rs`
6. `crates/stellatune-audio-core/src/decoder.rs`
7. `crates/stellatune-audio-core/src/transform.rs`
8. `crates/stellatune-audio-core/src/sink.rs`
9. `crates/stellatune-audio/src/planner.rs`
10. `crates/stellatune-audio/src/playback/control.rs`
11. `crates/stellatune-audio/src/playback/runtime.rs`
12. `crates/stellatune-audio/src/playback/actor.rs`
13. `crates/stellatune-audio/src/playback/preparation.rs`
14. `crates/stellatune-audio/src/playback/pump.rs`
15. `crates/stellatune-audio/src/playback/sink_worker.rs`
16. `crates/stellatune-backend-api/src/player_service/service.rs`
17. `crates/stellatune-backend-api/src/player_service/catalog.rs`
18. `crates/stellatune-backend-api/src/player_service/resolver.rs`
19. `crates/stellatune-backend-api/src/runtime/typescript_source.rs`

## 10. Module ownership and size policy

The playback control plane is owned by `control`, `event`, `runtime`, and
`actor`. Track and pipeline state lives in `state` and `preparation`; PCM work
lives in `pump`, `normalizer`, `transition`, and `sink_worker`. Dependencies
flow from orchestration toward these focused modules. Data-plane modules never
call the backend catalog, FFI, or UI.

Backend player identity/source/state records, catalog persistence, resolver
materialization, and service orchestration live in their corresponding
`player_service` submodules. Lyrics follows the same rule: `actor` owns message
state, `core` owns use-case orchestration, and `providers`, `cache`, and `parser`
own external I/O and representation details.

Every hand-written Rust file is limited to 1,200 physical lines, with roughly
900 lines as the growth target. `cargo run -p stellatune-xtask -- check-loc`
enforces the rule; only explicitly allowlisted generated files are skipped.

## Queue navigation and preparation ownership

For native playback, PlayerService is the sole owner of queue order, repeat,
shuffle, requested cursor, and observed cursor. Flutter projects these decisions;
it does not call next again on PlaybackEnded. DLNA transport retains its separate
remote-device navigation path.

Queue insertion allocates PlaybackItemId exactly once per occurrence. Repeated
tracks receive distinct item IDs. Selecting, prewarming, and restoring an item
use that existing ID; none of those operations append to the queue. Replacement
and removal are transactional. A full queue reports capacity exhaustion instead
of silently evicting a different occurrence. Repeat-one applies to automatic
succession; manual next advances in traversal order. Repeat-all wraps; sequential
and shuffle stop at the end. Appending retains the existing shuffle traversal.

The audio controller exposes three operations:

- switch_to(item, options) selects an explicit target. A matching successor is
  reused; otherwise the session's old preparations are invalidated.
- set_next(Some(item)) prepares one successor; set_next(None) clears its slot.
- advance_to_next(expected_item_id, options) atomically claims a ready or preparing
  successor. It returns Accepted, AlreadyCurrent, or Unavailable without a snapshot/check race.
  AlreadyCurrent handles a natural promotion that raced a manual next request.

NextTrack is Empty, Preparing(task identity, item identity, cancellation,
deadline), or Ready(pipeline). An active crossfade owns its secondary pipeline
separately. Advancement toward that secondary retains the overlap. Advancement
toward its successor waits until the current overlap releases the mixer.

Backend navigation updates requested_item_id before awaiting source resolution.
A cancellation token identifies each navigation intent. Resolver results recheck
that intent while holding the queue mutex through Lattice mailbox admission;
pipeline preparation is awaited after releasing the mutex. Consecutive next
commands therefore advance A -> requested B -> requested C even while B opens.

TrackChanged updates the observed cursor and starts preparing its successor.
Older boundaries cannot replace a newer requested target. Source resolution for
prewarm also has an independent cancellation token. Queue edits replace that
work only when the selected successor changes. Slow preparation never holds the
queue decision mutex. Periodic snapshots reconcile lagged event consumption and
persist runtime position; runtime objects and cancellation tokens are not stored.
