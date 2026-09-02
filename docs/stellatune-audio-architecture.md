# `stellatune-audio` Internal Architecture

This document describes the implemented playback boundaries. The complete
hard-switch design and acceptance criteria are in
[`player-core-refactor.md`](player-core-refactor.md).

## 1. Ownership and control

`PlaybackRuntime` owns the playback actor and sink-worker lifecycle. Cloneable
`PlaybackController` values are typed command endpoints; dropping a controller
does not stop the runtime.

```text
Flutter / TUI
    -> PlayerService (TrackId resolution and persistence)
    -> PlaybackController (PlaybackItem commands)
    -> PlaybackActor (state, generation, epoch, transition, recovery)
    -> bounded PCM channel
    -> SinkWorker (final gain, markers, device clock)
    -> SinkStage
```

The actor mailbox carries control and preparation completion messages only.
Encoded bytes and PCM never pass through FFI, JSON-RPC, or an actor mailbox.

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

Source open and decoder preparation run outside the actor. Every completion is
tagged with the current generation. Advancing the generation cancels the old
`SourceOpenRequest` before stale results are dropped. Decoder fallback is
limited to preparation and requires a reopenable source. The decoder never
opens a file or HTTP locator; those responsibilities stay in source factories.

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

## 4. Transitions and position

- Gapless prewarms next, trims encoder delay/padding, reuses compatible output,
  and applies no transition gain.
- FadeOutIn drives one track pipeline at a time with frame-based envelopes.
- Crossfade drives current and next simultaneously, applies independent gains
  before the mixer, and runs post-mix DSP once.

If next fails after overlap starts, the failure remains associated with the
next `PlaybackItemId`; the current pipeline keeps its pending PCM and ramps from
its instantaneous crossfade gain back to unity.

Item-boundary markers travel in order with PCM. `TrackChanged` is emitted only
after the sink clock consumes the marker. Public position and recovery
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

## 6. Application persistence

`PlaybackStateStore` belongs to `stellatune-backend-api`, not the audio crates.
It stores typed source, track, and playback-item identities plus queue/current
and media-time position under one strict schema fingerprint. Startup restore
re-resolves local paths and provider sources and rebuilds every runtime stage.
Temporary URLs, HTTP headers, stage instances, generations, and epochs are not
persisted. Unknown or partial player schemas fail without migration or repair.
State validation and current/position writes share one SQLite transaction. The
persisted queue is bounded and catalog deletion uses tombstones, so stable IDs
are never reassigned.

Flutter reads a native playback snapshot to project restored state into the UI.
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

## 9. Reading order

1. `crates/stellatune-audio-core/src/contracts.rs`
2. `crates/stellatune-audio-core/src/source.rs`
3. `crates/stellatune-audio-core/src/decoder.rs`
4. `crates/stellatune-audio-core/src/transform.rs`
5. `crates/stellatune-audio-core/src/sink.rs`
6. `crates/stellatune-audio/src/planner.rs`
7. `crates/stellatune-audio/src/playback.rs`
8. `crates/stellatune-backend-api/src/player_service.rs`
9. `crates/stellatune-backend-api/src/runtime/typescript_source.rs`
