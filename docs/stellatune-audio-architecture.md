# `stellatune-audio` Internal Architecture

This document describes the implemented playback and plugin boundaries for maintainers.

## 1. Ownership model

`EngineHandle` sends independent typed Lattice messages to one `PlaybackActor`.
The actor exclusively owns `PlaybackState`, the current request/checkpoint, and
one optional `PlaybackSession`. No second control actor owns playback state.

```text
Flutter / TUI
    |
    v
EngineHandle
    | typed ask / tell
    v
PlaybackActor (Lattice)
    |
    +-- PlaybackSession
    |      +-- TrackPipeline: SourceStage -> DecoderStage
    |      +-- OutputPipeline: TransformStage[]
    |      `-- SinkWorkerHandle
    |
    `-- bounded PumpAudio turns
                  |
                  v
             bounded audio ring -> SinkWorker -> SinkStage
```

The actor mailbox transports control messages only. Encoded media and PCM blocks
never pass through an actor mailbox.

## 2. Planning and construction

A playback request is resolved into a typed `SourcePlan`.
`PipelinePlanner` combines that plan with user policy and the immutable
`CapabilityRegistry` snapshot to produce an `ExecutablePlaybackPlan`.
`PipelineBuilder` and `PipelineFactory` then construct a complete session.

The registry stores descriptors and factories, never active stages or playback
state. Structural changes rebuild a session at a safe boundary. The only
in-place updates are explicit typed controls such as master gain, transition
gain, and gapless trim.

## 3. Stage and data-plane boundaries

The executable data plane only knows the Rust traits `SourceStage`,
`DecoderStage`, `TransformStage`, and `SinkStage`.

- Built-in stages execute directly in process.
- External native stages are represented by Rust proxy stages.
- External PCM uses a bounded shared-memory ring plus framed control IPC.
- The sink stage and device calls are owned by the dedicated `SinkWorker`.
- Backpressure is bounded; the playback actor retains at most the documented
  pending block while the sink ring is full.

ASIO remains an optional, separately built and distributed external sidecar.
The default workspace and Rust core do not link or distribute the ASIO SDK.

## 4. TypeScript control plane

TypeScript plugins are pre-bundled ESM loaded by a shared Node runner. A running
plugin has at most one lazily started process, shared by all of its capabilities.

TypeScript may resolve sources, search, authenticate, provide lyrics, and
perform network-device control. It returns declarative data such as
`SourcePlan`; Rust fetches media bytes and selects the final decoder,
transforms, and output. Node, JSON-RPC, and plugin UI routes never carry PCM.

Plugin packages use Manifest v2 and contain no install scripts or native addons.
The first implementation assumes first-party or explicitly trusted local code;
it does not expose a permissions mini-language.

## 5. Plugin changes

Install, update, enable, disable, and uninstall use one deterministic sequence:

1. ask `PlaybackActor` to suspend and capture the actual consumed position;
2. tear down the complete active session and stop the affected plugin process;
3. stage and atomically commit the package/catalog change;
4. publish the new registry snapshot;
5. rebuild from the checkpoint with the new capability set;
6. resume only if playback was active before the change.

There are no active-stage leases, delayed uninstall, concurrent package
generations, or graph mutations.

## 6. Playback behavior

Seek, EOF promotion, bounded recovery, and plugin-change restoration are owned
by `PlaybackSession` under `PlaybackActor`.

A queued next track may be prewarmed. At EOF, a compatible prewarmed track is
promoted; otherwise a new track pipeline is built. Output reuse is decided from
typed compatibility data. Sink disconnect recovery tears down the failed
session, retries with bounded backoff, and either restores the checkpoint or
emits a terminal error.

## 7. Runtime topology

- Lattice schedules the playback, plugin-manager, TypeScript-process, library,
  filesystem-watch, and lyrics control actors.
- `PlaybackActor` uses the pinned single-worker dedicated execution policy.
- `SinkWorker` remains a dedicated device thread with a bounded audio ring.
- Tokio owns ordinary asynchronous I/O and background tasks.
- No project-specific Actor runtime or unified command enum exists.

## 8. Reading order

1. `crates/stellatune-audio/src/engine/actor.rs`
2. `crates/stellatune-audio/src/engine/messages.rs`
3. `crates/stellatune-audio/src/engine/session.rs`
4. `crates/stellatune-audio/src/pipeline/plan.rs`
5. `crates/stellatune-audio/src/pipeline/capability.rs`
6. `crates/stellatune-audio/src/pipeline/runtime/runner/mod.rs`
7. `crates/stellatune-audio/src/workers/sink/worker.rs`
8. `crates/stellatune-backend-api/src/runtime/plugin_manager.rs`
9. `crates/stellatune-plugins/src/typescript/process.rs`
