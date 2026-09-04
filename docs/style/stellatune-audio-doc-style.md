# Stellatune Audio Rustdoc Style Guide

This guide defines the rustdoc style for `stellatune-audio-core` and
`stellatune-audio`. Documentation is part of the contract: both crates deny
missing public documentation and broken intra-doc links.

## Language and Tone

1. Use English only.
2. Keep a neutral, technical tone.
3. Prefer precise behavior statements over implementation narration.
4. Avoid marketing language.

## Required Structure

For crate and module documentation:

1. State what the crate or module owns and what it deliberately does not own.
2. Explain the data flow, lifecycle, or invariants needed to read the code.
3. Link to the next relevant public module or type.
4. Avoid duplicating the system architecture document line for line.

For public functions and methods:

1. First line: one-sentence summary, imperative avoided, period required.
2. Main body: behavior and side effects.
3. `# Errors`: required when return type includes `Result`.
4. `# Panics`: required only when callers can trigger a panic under the
   documented contract.
5. `# Examples`: required for high-traffic APIs, preferred elsewhere.

For public enums/structs:

1. Start with role/purpose.
2. Document invariants and semantic constraints.
3. For enum variants, describe when each variant is produced.

For private/internal functions:

1. Do not document obvious wrappers/getters/setters.
2. Document cross-module `pub(super)` interfaces when they mutate session
   state, advance a pipeline, perform recovery, or cross a thread boundary.
3. Add private docs when behavior is non-trivial (state machine, threading,
   recovery, ordering, invariants, or error policy).
4. Prefer short function-level intent + selective inline comments at critical
   branches over verbose line-by-line narration.

## Contracts and Units

1. Name the coordinate system for every frame counter: decoded, audible,
   mixed, queued, or device-consumed.
2. State units for durations, capacities, positions, and identifiers.
3. For async operations, document deadlines, cooperative cancellation, and
   whether dropping the caller's future cancels the underlying operation.
4. For stage traits, document construction/configuration order, valid status
   transitions, backpressure, and reset/drain responsibilities.
5. Describe observable behavior and invariants; avoid promising incidental
   implementation details that callers cannot rely on.

## Linking and Terminology

1. Use intra-doc links for crate types, for example:
   - [`crate::playback::PlaybackController`]
   - [`stellatune_audio_core::error::PlaybackControlError`]
2. Keep terms stable across docs:
   - "PlaybackActor"
   - "preparation task"
   - "track pipeline"
   - "SinkWorker"

## Examples

1. Every example must compile as a doctest; do not use `ignore` or ellipses in
   place of required code.
2. Use runnable examples when behavior is deterministic and self-contained.
3. For runtime APIs that require application wiring, define a function that
   accepts an existing controller, registry, item, or runtime.
4. Use `no_run` only when a complete example depends on external state.
5. Keep examples minimal and focused on one concept.

## Error Documentation Rules

1. Map concrete failure cases to concrete error variants where practical.
2. Do not document errors as generic "operation failed".
3. If an error is wrapped from another layer, name that layer explicitly.

## Formatting Rules

1. Keep summary line short (roughly one sentence).
2. Prefer short paragraphs over large narrative blocks.
3. Use bullet lists for multiple conditions or guarantees.
4. Avoid comment noise: if a reader can infer behavior immediately from code,
   skip the doc comment.

## Review Checklist

- Does the crate or module page explain ownership and navigation?
- Is the first sentence a complete summary?
- Are semantics and side effects clear?
- Are units and frame coordinate systems explicit?
- Is `# Errors` present and specific for fallible APIs?
- Is `# Panics` present only when applicable?
- Are links and terms consistent with this guide?
- Does every example compile and match the documented contract?
- For private docs, do comments focus on complex behavior instead of obvious code?

## Local Validation

```text
RUSTDOCFLAGS="-D warnings" cargo doc -p stellatune-audio-core -p stellatune-audio --no-deps --document-private-items
cargo test -p stellatune-audio-core -p stellatune-audio --doc
```
