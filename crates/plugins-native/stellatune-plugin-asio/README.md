# stellatune-plugin-asio

ASIO output sink plugin for StellaTune (sidecar mode).

This plugin does not talk to ASIO directly in-process. It launches the
`stellatune-asio-host` sidecar process and streams PCM through a shared ring
buffer.

## Why sidecar

- Keeps ASIO/CPAL host logic isolated from player process.
- Reduces crash blast radius of driver/backend faults.
- Reuses existing `stellatune-asio-proto` protocol and shared memory ring.

## Runtime requirements

- Windows
- `stellatune-asio-host` built with feature `asio`
- Sidecar executable available under plugin runtime root:
  - `stellatune-asio-host.exe`, or
  - `bin/stellatune-asio-host.exe`

You can override the sidecar path with plugin config `sidecar_path`.

## Build sidecar

From repository root:

```powershell
cargo build --manifest-path crates/plugins-native/stellatune-asio-host/Cargo.toml --features asio --release
```

Then copy the produced `stellatune-asio-host.exe` into plugin runtime root
(or `bin/` under runtime root).

## One-shot packaging (recommended)

Use the packaging script to build plugin + sidecar and produce an installable zip:

```powershell
powershell -ExecutionPolicy Bypass -File crates/plugins-native/stellatune-plugin-asio/scripts/package-windows.ps1
```

Optional flags:

```powershell
# Explicit target
powershell -ExecutionPolicy Bypass -File crates/plugins-native/stellatune-plugin-asio/scripts/package-windows.ps1 -Target x86_64-pc-windows-msvc

# Debug build output
powershell -ExecutionPolicy Bypass -File crates/plugins-native/stellatune-plugin-asio/scripts/package-windows.ps1 -Configuration Debug

# Custom artifact directory
powershell -ExecutionPolicy Bypass -File crates/plugins-native/stellatune-plugin-asio/scripts/package-windows.ps1 -OutDir .\artifacts\plugins
```

The script creates a zip artifact containing:

- `stellatune_plugin_asio.dll` (plugin)
- `bin/stellatune-asio-host.exe` (ASIO sidecar)

Install the generated zip from StellaTune Settings -> Plugins -> Install.
Installer will unpack it into plugin runtime root, and the plugin can resolve
the sidecar automatically.

## Plugin config

```json
{
  "sidecar_path": null,
  "sidecar_args": [],
  "buffer_size_frames": null,
  "sample_rate_mode": "fixed_target",
  "fixed_target_sample_rate": null,
  "ring_capacity_ms": 250,
  "start_prefill_ms": 0,
  "preferred_chunk_frames": 0,
  "latency_profile": "balanced",
  "flush_timeout_ms": 400
}
```

Field notes:

- `sidecar_path`: absolute path or runtime-root-relative path.
- `buffer_size_frames`: passed to sidecar `Open` request.
- `sample_rate_mode`:
  - `fixed_target`: keep one negotiated output sample rate (recommended for lessgap).
  - `match_track`: follow each track sample rate (may cause more reopen/re-negotiate events).
- `fixed_target_sample_rate`: used in `fixed_target` mode. When explicitly set, plugin forces this exact output rate (does not auto-fallback to nearest caps rate). `null` means device default sample rate.
- `ring_capacity_ms`: shared ring capacity in milliseconds.
- `latency_profile`: ASIO buffering aggressiveness preset:
  - `aggressive`: lower latency, higher underrun risk.
  - `balanced`: middle ground.
  - `conservative`: higher latency, better startup/switch stability.
- `start_prefill_ms`: sidecar stream start prefill threshold. `0` means auto by `latency_profile` (`aggressive`=15ms, `balanced`=30ms, `conservative`=60ms).
- `preferred_chunk_frames`: host write chunk hint via negotiation. `0` means auto by sample rate and `latency_profile` (base: 48k->128, 96k->256, 192k->512; then `aggressive` x1, `balanced` x2, `conservative` x4). `>0` uses fixed chunk size.
- `flush_timeout_ms`: best-effort flush wait timeout before close.

## Limitations

- Windows only.
- ASIO availability depends on `stellatune-asio-host` build flags and local
  driver state.
