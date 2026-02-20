# Hot-Path Core ABI v1 (Frozen 2026-02-20)

This document defines the low-level core Wasm ABI used when a plugin returns
`some(core-module-spec)` from:

- `output-sink.session.describe-hot-path(...)`
- `dsp.processor.describe-hot-path(...)`

The goal is near-zero-copy processing on the host<->plugin boundary for
realtime-heavy paths.

Freeze rule:
- This spec is locked as `abi_version = 1` for the SDK phase.
- Breaking layout/signature updates require `abi_version` increment.

## 1. Scope

- In scope:
  - `output-sink` hot path
  - `dsp` hot path
- Out of scope (for now):
  - decoder hot path
  - source hot path

## 2. Module Requirements

- Core Wasm module (not component) targeting `wasm32`.
- Must export linear memory named by `core-module-spec.memory-export`.
- Must export functions named by:
  - `core-module-spec.init-export`
  - `core-module-spec.process-export`
  - optional `core-module-spec.reset-export`
  - optional `core-module-spec.drop-export`
- `abi-version` must match host-supported ABI version.

## 3. Integer ABI Convention

- All pointers are `u32` offsets into exported linear memory.
- All lengths/counts are `u32`.
- Return code: `i32` (`0` success, non-zero error).
- Host and guest use little-endian encoding.

## 4. Required Exports

## `init`

Signature:

```c
// returns 0 on success
// out_ctx_ptr points to a u32 in module memory written by guest
int32_t st_hot_init(uint32_t args_ptr, uint32_t out_ctx_ptr);
```

- `args_ptr` points to `st_hot_init_args` in module memory.
- Guest writes an opaque context handle to `out_ctx_ptr`.

## `process`

Signature:

```c
// returns 0 on success
int32_t st_hot_process(
    uint32_t ctx,
    uint32_t frames,
    uint32_t out_frames_ptr,
    uint32_t out_flags_ptr
);
```

- `frames` is requested frame count (`<= max_frames`).
- Guest writes produced/consumed frames to `out_frames_ptr`.
- Guest writes status flags to `out_flags_ptr`.

## `reset` (optional)

```c
int32_t st_hot_reset(uint32_t ctx, uint32_t flags);
```

## `drop` (optional but strongly recommended)

```c
void st_hot_drop(uint32_t ctx);
```

If not exported, host releases resources by instance teardown.

## 5. Init Arg Struct Layout

Host writes this struct to module memory before calling `init`.

```c
typedef struct st_hot_init_args {
  uint32_t abi_version;      // must equal 1
  uint32_t role;             // 1=dsp-transform, 2=output-sink
  uint32_t sample_rate;      // Hz
  uint16_t channels;         // interleaved channels
  uint16_t sample_format;    // 1=f32le, 2=i16le, 3=i32le
  uint32_t max_frames;       // max frames per process call

  uint32_t in_offset;        // byte offset in memory
  uint32_t out_offset;       // byte offset in memory, 0 allowed for sink role
  uint32_t buffer_bytes;     // size of each buffer region in bytes

  uint32_t flags;            // init behavior flags
  uint32_t reserved0;
  uint32_t reserved1;
} st_hot_init_args;
```

Role mapping:

- `dsp-transform`: `in_offset` and `out_offset` must be valid regions.
- `output-sink`: `in_offset` must be valid; `out_offset` may be `0`.

## 6. Process Flags

`out_flags_ptr` bitmask written by guest:

- `1 << 0` (`ST_HOT_FLAG_EOF`): no more data expected.
- `1 << 1` (`ST_HOT_FLAG_DRAINED`): internal queue drained.
- `1 << 2` (`ST_HOT_FLAG_NEED_RESET`): guest requests reset boundary.
- `1 << 3` (`ST_HOT_FLAG_SOFT_ERROR`): non-fatal issue, fallback permitted.

## 7. Error Codes

Recommended non-zero codes:

- `1`: invalid arg
- `2`: unsupported
- `3`: io
- `4`: internal
- `5`: would-block
- `6`: not-ready

Host maps codes to `plugin-error` and runtime diagnostics.

## 8. Host Call Sequence

1. Component control path:
  - call `describe-hot-path(spec)`
  - if `none`: fallback to normal component methods
2. Load core module from `wasm-rel-path`.
3. Resolve memory/exported functions.
4. Allocate/init shared regions in module memory.
5. Write `st_hot_init_args`, call `init`.
6. Realtime loop:
  - write input frames to `in_offset` region
  - call `process`
  - read `out_frames` + `out_flags`
  - for DSP: read output from `out_offset`
7. Route change / disruption:
  - call `reset` when available
8. Disable/unload:
  - call `drop` when available
  - teardown instance/memory

## 9. Realtime Safety Rules

- No `memory.grow` during active processing.
- No blocking I/O inside `process`.
- No dynamic allocation in `process` if avoidable.
- No logging/syscall/sidecar startup inside `process`.
- Bound `process` runtime to callback budget.

## 10. Compatibility and Versioning

- This spec defines `abi_version = 1`.
- Any breaking layout/signature change increments ABI version.
- Host must reject mismatched versions and fallback to component path.
