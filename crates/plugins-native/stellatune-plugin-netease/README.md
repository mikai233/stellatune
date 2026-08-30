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
powershell -ExecutionPolicy Bypass -File scripts/package-windows.ps1
```

The archive contains only `manifest.json`, `plugin.mjs`, its configuration
schema, and static Web UI assets. It contains no embedded executable, native addon,
`node_modules`, or install script.
