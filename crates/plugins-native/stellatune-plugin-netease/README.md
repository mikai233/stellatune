# Netease Cloud Music plugin

This is a Manifest v2 control-plane plugin. Its pre-bundled ESM module provides
source resolution, search/library, authentication, and lyrics capabilities via
the shared Node runner.

Media bytes do not pass through Node. The resolver returns an HTTP `SourcePlan`;
Stellatune opens that URL and decodes it with the native Rust Symphonia stage.
The optional Netease service remains a separately managed local process and is
not embedded in the plugin archive.

Package on Windows:

```powershell
npm --prefix ui-web ci
powershell -ExecutionPolicy Bypass -File scripts/package-windows.ps1
```

The archive contains only `manifest.json`, `plugin.mjs`, its configuration
schema, and static Web UI assets. It contains no embedded executable, native addon,
`node_modules`, or install script.

The packaging script builds both the Vue UI and the Node bundle. Source lives
in `src/`; SDK helpers are bundled into `plugin.mjs`. Install the resulting
`dist/dev.stellatune.source.netease-0.3.0.zip` with the updated host.

The plugin hosts its own UI on a random loopback port. Open it from Settings;
the plugin process stays alive until disabled, updated, uninstalled or app exit.
Its configuration is `plugin-data/dev.stellatune.source.netease/config.json`
under the app support directory. Configure the sidecar through the plugin UI;
search, login and playback resolution share this persisted configuration.
Old WebUI/Flutter configuration is not imported. The sidecar is still external.

For Vite development, set `STELLATUNE_NETEASE_UI_DEV_URL=http://127.0.0.1:5173`
before starting the app. Open the plugin UI, read its API URL from the plugin
stderr log, set `STELLATUNE_NETEASE_UI_API` to that URL in the Vite terminal,
and run `npm --prefix ui-web run dev`. Vite proxies `/api` to the plugin.
Neither UI nor host calls need tokens.

Validation:

```powershell
npm --prefix ui-web run build
npm --prefix ui-web run check:sdk
npm --prefix ui-web test
```

The integration test copies the bundle and assets into a temporary installation,
uses local host/sidecar fixtures, and checks UI, persisted configuration, login,
source resolution, player commands, event forwarding and shutdown.
