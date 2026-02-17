# `stellatune-audio` Internal Architecture

This document describes the runtime architecture of `crates/stellatune-audio` for maintainers.
It complements rustdoc by focusing on cross-module behavior, ownership boundaries, and failure flow.

## 1. Component Map

- `engine`:
  - Hosts the control actor and the public control surface (`EngineHandle`).
  - Serializes high-level commands and forwards work to worker components.
- `workers::decode`:
  - Owns playback state and the active `PipelineRunner`.
  - Implements EOF transition policy and sink-recovery policy.
- `pipeline::runtime::runner`:
  - Drives source/decoder/transform stages.
  - Maintains runtime control routes and sink-session compatibility checks.
- `workers::sink`:
  - Owns sink-stage I/O thread.
  - Consumes audio blocks from a bounded ring and serves sink control RPCs.
- `pipeline::runtime::sink_session`:
  - Manages sink worker lifetime, route fingerprinting, and reuse decisions.

## 1.1 External Boundaries

`stellatune-audio` is runtime-core only. It does not own plugin capability
binding directly. Integration boundaries are:

- `crates/audio-adapters/stellatune-audio-plugin-adapters`:
  - Plugin-backed source/decoder/transform/output stage adapters.
- `crates/audio-adapters/stellatune-audio-builtin-adapters`:
  - Built-in device/output runtime adapters.
- `crates/stellatune-backend-api`:
  - Assembler/runtime wiring and app-facing orchestration.

## 2. Thread Topology

The runtime uses a small fixed topology:

1. Control actor thread:
   - Receives external commands.
   - Performs orchestration and sends worker commands.
2. Decode worker thread:
   - Owns active runner and decode loop state.
   - Executes command handlers and periodic step/recovery ticks.
3. Sink worker thread:
   - Owns sink stage instances.
   - Writes queued blocks and executes control calls (`drain`, `drop_queued`, `sync_runtime_control`).

This split keeps high-frequency audio movement out of actor command channels.

The exact sink implementation (builtin output device vs plugin output sink) is
selected by pipeline assembly and sink route state, not by the runner loop.

## 3. Data Plane vs Control Plane

- Data plane:
  - `PipelineRunner::step` produces an `AudioBlock`.
  - `SinkSession` forwards it to `SinkWorker::try_send_block`.
  - `SinkWorker` drains ring-buffered blocks and writes to sink stages.
- Control plane:
  - Actor commands (open/play/seek/stop/mutations) go to decode worker mailbox.
  - Runtime-control synchronization checkpoints run in `PipelineRunner::sync_runtime_control`.
  - Sink control calls are RPC-like commands to the sink thread mailbox.

The two planes intentionally converge at explicit checkpoints (runner step, sink control handlers).

## 4. Track Transition Flow

EOF transition in decode loop follows this order:

1. Promote prewarmed next runner if available.
2. Otherwise open queued-next input.
3. Otherwise stop with drain semantics and emit EOF.

This ordering optimizes for sink-route reuse and low-latency handoff when prewarming is available.

## 5. Sink Disconnect Recovery

When runner stepping returns `SinkDisconnected`:

1. Decode worker tears down the active runner.
2. Recovery state is scheduled with bounded retries and exponential backoff.
3. Rebuild attempts reassemble the runner and reactivate sink session.
4. On exhaustion, decode worker emits an error and stops playback.

Recovery policy is intentionally decode-worker-owned so retry visibility and state transitions remain centralized.

## 6. Runtime Invariants

- A runner must be decode-prepared and sink-prepared before stepping.
- Stage control dispatch is stage-key based and validated at runner construction.
- At most one pending sink block is retained in runner as a backpressure bridge.
- Sink thread owns sink stage calls; non-sink threads do not invoke sink stage methods directly.

## 7. API Boundary Notes

- Engine/public control methods are asynchronous actor messages.
- Decode and sink workers are internal execution details and are not public API.
- Runtime errors crossing crate boundary are normalized to typed engine/decode
  errors in `stellatune-audio`, then adapted again by upper layers.

## 8. Reading Order for Maintainers

Recommended file reading order:

1. `crates/stellatune-audio/src/engine/mod.rs`
2. `crates/stellatune-audio/src/workers/decode/mod.rs`
3. `crates/stellatune-audio/src/workers/decode/loop.rs`
4. `crates/stellatune-audio/src/pipeline/runtime/runner/mod.rs`
5. `crates/stellatune-audio/src/pipeline/runtime/runner/step.rs`
6. `crates/stellatune-audio/src/workers/sink/worker.rs`
