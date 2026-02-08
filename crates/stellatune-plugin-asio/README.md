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
cargo build --manifest-path crates/stellatune-asio-host/Cargo.toml --features asio --release
```

Then copy the produced `stellatune-asio-host.exe` into plugin runtime root
(or `bin/` under runtime root).

## One-shot packaging (recommended)

Use the packaging script to build plugin + sidecar and produce an installable zip:

```powershell
powershell -ExecutionPolicy Bypass -File crates/stellatune-plugin-asio/scripts/package-windows.ps1
```

Optional flags:

```powershell
# Explicit target
powershell -ExecutionPolicy Bypass -File crates/stellatune-plugin-asio/scripts/package-windows.ps1 -Target x86_64-pc-windows-msvc

# Debug build output
powershell -ExecutionPolicy Bypass -File crates/stellatune-plugin-asio/scripts/package-windows.ps1 -Configuration Debug

# Custom artifact directory
powershell -ExecutionPolicy Bypass -File crates/stellatune-plugin-asio/scripts/package-windows.ps1 -OutDir .\artifacts\plugins
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
  "ring_capacity_ms": 250,
  "preferred_chunk_frames": 256,
  "flush_timeout_ms": 400
}
```

Field notes:

- `sidecar_path`: absolute path or runtime-root-relative path.
- `buffer_size_frames`: passed to sidecar `Open` request.
- `ring_capacity_ms`: shared ring capacity in milliseconds.
- `preferred_chunk_frames`: host write chunk hint via negotiation.
- `flush_timeout_ms`: best-effort flush wait timeout before close.

## Limitations

- Windows only.
- ASIO availability depends on `stellatune-asio-host` build flags and local
  driver state.
