# stellatune-plugin-netease

Wasm source plugin for NetEase Cloud Music.

This plugin provides:

- Source type: `netease`
- Decoder type: `stream_symphonia`
- Plugin WebUI (Vue 3 + Vite + TypeScript): `ui/index.html`

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

## Plugin UI action contract

The plugin WebUI can call gateway actions:

- `netease.auth.login_status`
- `netease.auth.login_refresh`
- `netease.auth.qr.start`
- `netease.auth.qr.status`
- `netease.auth.logout`

Gateway dispatches custom actions to source runtime using `list_items_json` with request shape:

```json
{
  "action": "netease.auth.qr.status",
  "key": "optional_qr_key",
  "cookie": "optional_ncm_cookie"
}
```

Action response is returned as:

```json
{
  "dispatch": "source.list_items_json",
  "source_type_id": "netease",
  "response": [
    {
      "kind": "control_result",
      "item_id": "netease.auth.qr.start",
      "playlist_ref": {
        "key": "qr_key_here",
        "create_payload": {}
      }
    }
  ]
}
```

## Build

```powershell
cargo build --manifest-path crates/plugins-native/stellatune-plugin-netease/source/Cargo.toml --target wasm32-wasip2 --release
cargo build --manifest-path crates/plugins-native/stellatune-plugin-netease/decoder/Cargo.toml --target wasm32-wasip2 --release
```

## Plugin WebUI (dev/build)

```powershell
cd crates/plugins-native/stellatune-plugin-netease/ui-web
npm install
npm run dev
```

Build static assets to plugin package path:

```powershell
cd crates/plugins-native/stellatune-plugin-netease/ui-web
npm run build
```

This writes files into `crates/plugins-native/stellatune-plugin-netease/ui/`.

## Plugin WebUI dev mode (framework-agnostic)

For fast iteration, you can map plugin UI to any local dev server (Vue/React/Svelte/etc):

```powershell
$env:STELLATUNE_PLUGIN_UI_DEV_ORIGINS='{"dev.stellatune.source.netease":"http://127.0.0.1:5173"}'
```

Then start host and open normal gateway URL:

```text
http://127.0.0.1:<gateway-port>/ui/dev.stellatune.source.netease?token=<token>
```

Gateway behavior:

- `/ui/:plugin_id` redirects to your configured local `dev_origin`.
- Redirect injects `token`, `plugin_id`, and `gateway_origin` query params.
- WebUI can call gateway API directly using `gateway_origin` (no framework-specific proxy required).
- Gateway API CORS allowlist automatically includes configured dev origins.

## Packaging (Windows)

```powershell
powershell -ExecutionPolicy Bypass -File crates/plugins-native/stellatune-plugin-netease/scripts/package-windows.ps1
```

The zip contains:

- `plugin.json`
- `wasm/stellatune_plugin_netease_source.wasm`
- `wasm/stellatune_plugin_netease_decoder.wasm`
- `ui/*` (plugin web UI static assets)
- `bin/stellatune-ncm-sidecar.exe`
