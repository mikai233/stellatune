# stellatune-plugin-netease

Wasm source plugin for NetEase Cloud Music.

This plugin provides:

- Source type: `netease`
- Decoder type: `stream_symphonia`

## Runtime model

- The source component runs as Wasm (`source-plugin` world).
- The component launches and reuses a sidecar process through host `sidecar` imports.
- Sidecar HTTP base URL default: `http://127.0.0.1:46321`.
- Sidecar implementation is under `tools/stellatune-ncm-sidecar`.

## Plugin config

```json
{
  "sidecar_base_url": "http://127.0.0.1:46321",
  "sidecar_path": null,
  "sidecar_args": [],
  "api_request_timeout_ms": 8000,
  "stream_read_timeout_ms": null,
  "default_level": "standard"
}
```

## Build

```powershell
cargo build --manifest-path crates/plugins-native/stellatune-plugin-netease/source/Cargo.toml --target wasm32-wasip2 --release
cargo build --manifest-path crates/plugins-native/stellatune-plugin-netease/decoder/Cargo.toml --target wasm32-wasip2 --release
```

## Packaging (Windows)

```powershell
powershell -ExecutionPolicy Bypass -File crates/plugins-native/stellatune-plugin-netease/scripts/package-windows.ps1
```

The zip contains:

- `plugin.json`
- `wasm/stellatune_plugin_netease_source.wasm`
- `wasm/stellatune_plugin_netease_decoder.wasm`
- `bin/stellatune-ncm-sidecar.exe`
