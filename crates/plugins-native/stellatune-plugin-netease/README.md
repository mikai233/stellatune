# stellatune-plugin-netease

Netease source plugin for StellaTune (development stage).

This plugin currently provides:

- `SourceCatalog` type: `netease`
- Decoder type: `stream_symphonia`

## Runtime model

- The plugin talks to a sidecar HTTP service.
- Sidecar default URL: `http://127.0.0.1:46321`.
- Sidecar implementation is under `tools/stellatune-ncm-sidecar`.

## Plugin config

```json
{
  "sidecar_base_url": "http://127.0.0.1:46321",
  "sidecar_path": null,
  "sidecar_args": [],
  "request_timeout_ms": 8000,
  "default_level": "standard"
}
```

## Build

```powershell
cargo build --manifest-path crates/plugins-native/stellatune-plugin-netease/Cargo.toml --release
```

## Packaging

```powershell
powershell -ExecutionPolicy Bypass -File crates/plugins-native/stellatune-plugin-netease/scripts/package-windows.ps1
```

The zip contains:

- `stellatune_plugin_netease.dll`
- `bin/stellatune-ncm-sidecar.exe` (standalone sidecar executable, no Node.js runtime required on end-user machine)
