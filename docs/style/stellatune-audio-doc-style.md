# `stellatune-audio` Documentation Style Guide

This guide defines the rustdoc style for `crates/stellatune-audio`.

## Language and Tone

1. Use English only.
2. Keep a neutral, technical tone.
3. Prefer precise behavior statements over implementation narration.
4. Avoid marketing language.

## Required Structure

For public functions and methods:

1. First line: one-sentence summary, imperative avoided, period required.
2. Main body: behavior and side effects.
3. `# Errors`: required when return type includes `Result`.
4. `# Examples`: required for high-traffic APIs, preferred elsewhere.

For public enums/structs:

1. Start with role/purpose.
2. Document invariants and semantic constraints.
3. For enum variants, describe when each variant is produced.

## Linking and Terminology

1. Use intra-doc links for crate types, for example:
   - [`crate::engine::EngineHandle`]
   - [`crate::error::EngineError`]
2. Keep terms stable across docs:
   - "control actor"
   - "decode worker"
   - "pipeline runtime"
   - "sink session"

## Examples

1. Use `no_run` when examples depend on runtime wiring or external state.
2. Use runnable examples when behavior is deterministic and self-contained.
3. Keep examples minimal and focused on one concept.

## Error Documentation Rules

1. Map concrete failure cases to concrete error variants where practical.
2. Do not document errors as generic "operation failed".
3. If an error is wrapped from another layer, name that layer explicitly.

## Formatting Rules

1. Keep summary line short (roughly one sentence).
2. Prefer short paragraphs over large narrative blocks.
3. Use bullet lists for multiple conditions or guarantees.

## Review Checklist

- Is the first sentence a complete summary?
- Are semantics and side effects clear?
- Is `# Errors` present and specific for fallible APIs?
- Are links and terms consistent with this guide?
- Does the example match the documented contract?
