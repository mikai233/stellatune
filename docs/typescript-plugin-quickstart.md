# TypeScript Plugin Quickstart

StellaTune TypeScript plugins run in the control plane. They can resolve sources,
search, authenticate, provide lyrics, and control network services. They cannot
decode or process PCM and cannot choose the user's DSP or output device.

## Package layout

A package is a ZIP containing pre-bundled files. StellaTune never runs
`npm install` inside an installed package.

```text
manifest.json
plugin.mjs
config.schema.json       # optional
ui/                      # optional assets served by the plugin
```

Use Manifest v2:

```json
{
  "manifest_version": 2,
  "id": "dev.example.source",
  "name": "Example Source",
  "version": "0.1.0",
  "runtime": {
    "kind": "typescript",
    "entry": "plugin.mjs",
    "api_version": 2,
    "protocol": "stellatune-capability-rpc/1"
  },
  "capabilities": [{
    "id": "example-source",
    "kind": "source-resolver",
    "execution_class": "control",
    "display_name": "Example Source"
  }]
}
```

## Plugin entry

Bundle the entry as ESM and export a plugin object. The SDK types live in
`tools/typescript-plugin-runtime/sdk.ts`.

```ts
import { definePlugin } from "./sdk";

export default definePlugin({
  descriptor: {
    id: "dev.example.source",
    apiVersion: 2,
    capabilities: ["example-source"],
  },

  async invoke(request) {
    if (
      request.capabilityId === "example-source" &&
      request.operation === "resolve"
    ) {
      return {
        source: {
          kind: "http",
          url: "https://media.example.test/track.flac",
          headers: {},
        },
        media: { codecHint: "flac" },
        capabilities: { seekable: true },
        requirements: {},
      };
    }
    throw Object.assign(new Error("unsupported operation"), {
      code: "unsupported_operation",
      retryable: false,
    });
  },
});
```

The returned object is untrusted `SourceResolutionInput`. Rust validates it into
a `ResolvedSourceSpec`, binds a native `SourceFactory`, selects the decoder and
transforms, and sends PCM to the selected native output. Provider keys and the
temporary locator do not enter the audio core.

## Runtime and lifecycle

The application ships one Node runner. Each active plugin gets at most one
lazy process, shared by all capabilities and instances. Requests use framed
JSON-RPC with a generation and deadline. A plugin should release child processes
and other resources in its optional `shutdown` hook.

Plugins with a UI declare `"ui": { "mode": "plugin-hosted" }` in the manifest
and implement `openUi(): Promise<{ url: string }>`. The host calls it through
`plugin.open_ui` and opens the returned HTTP URL in the user's browser. Static
`ui.entry` manifests must be updated; there is no host static-UI gateway.

`initialize` receives `pluginId`, `generation`, `packageRoot`, `dataDir`, and
`hostApiBaseUrl`. `dataDir` is `<application-support>/plugin-data/<plugin-id>`;
it survives plugin updates and uninstall. Read and validate configuration there.
The native catalog and Flutter no longer send configuration snapshots. Existing
`.ui-config.json` and Flutter source configuration are ignored, not imported.
Changes affect subsequent capability calls, not already prepared audio.

UI opening must be idempotent. Use `startUiServer` from
`tools/typescript-plugin-runtime/ui-server.mjs` for an optional loopback static
server and supply your own business handler. Bundle helpers into `plugin.mjs`.
Processes start on first use and remain resident until disable, update,
uninstall or app exit. There is no idle collection or UI-specific keepalive.
Release HTTP listeners, SSE subscriptions and owned sidecars in `shutdown`.
After a crash, opening the UI again creates a new process and URL.

## Host control API

The application starts a loopback HTTP server before plugin initialization.
Playback restoration runs after plugin registration, so persisted plugin tracks
resolve with the initialized host context and the plugin's current configuration.
There are no authentication tokens, Origin allowlists or permission declarations.
The API has open CORS responses for local debugging.

| Endpoint | Purpose |
| --- | --- |
| `GET /health` | Readiness |
| `GET /player/state` | State, item/track identity, position and duration |
| `GET /player/queue` | Queue, cursor, mode and revision |
| `POST /player/commands` | Typed player commands |
| `GET /player/events` | Real player/queue events over SSE |

```ts
import { createHostClient } from "./sdk";
const host = createHostClient(context.hostApiBaseUrl);
await host.pause();
await host.command({ command: "playProviderTrack", track: {
  pluginId: context.pluginId, capabilityId: "example-source",
  providerId: "example-account", providerKey: "42",
} });
const unsubscribe = host.subscribe(event => console.log(event));
```

Use camelCase fields and decimal **strings** for 64-bit identities and revisions.
Times are milliseconds. Command types and event types are exported by the SDK.
Unknown commands and invalid input return 400, absent targets 404, unavailable
services 503, and internal failures 500, with `{ code, message }` errors.
Command replies follow player acknowledgement semantics; actual playback state
comes from events. The host exposes no arbitrary plugin-action dispatch.

SSE starts with a snapshot after subscriptions are established. A lagged client
receives `resync`; the SDK fetches state and queue again. On reconnect a fresh
snapshot replaces previous state. Subscriptions are not persisted or replayed.
Plugin pages normally use their own Node service, which forwards host events.

Native catalog browsing invokes an explicitly selected `network-control`
capability with `list-items`. Returned track rows include
`source_resolver_capability_id` so playback selects the exact source resolver.
No capability is guessed from an action name.

## Optional local container plugins

A source resolver may declare `"local_extensions": ["example"]`. Extensions
are lowercase without dots. For files outside the native decoder's formats,
the host selects an enabled resolver by extension and calls:

- `inspect-file`, input `{ path }`: return `{ title, artist, album, durationMs }`
  for library scanning. This operation should only inspect the container header.
- `resolve-file`, input `{ path }`: return a `SourcePlan` with an absolute file
  path or HTTP URL, media hints and seek capabilities. HTTP servers should
  provide Content-Length, Accept-Ranges and correct 206/416 responses for seek.

The host supplies the canonical original file path. The plugin owns its source;
the generic Rust decoder reads the returned file or HTTP stream. Duplicate enabled
extension owners cause an explicit error. The library retains the original
path and track identity. Disabling the plugin removes future format resolution
and scan support, and shuts down any HTTP source server owned by that plugin.

The separately packaged `stellatune-plugin-ncm` demonstrates this flow. It
uses a separately packaged Rust executable with ncmdump to decrypt requested
byte ranges over loopback HTTP. It writes no decoded/decrypted media caches and
is not bundled with the main application. NCM-specific code and dependencies
exist only in that executable. See its README for building and installation.

## Trust model

Manifest v2 intentionally has no generic permissions list. Installed code is
treated as first-party or explicitly trusted local code, not as sandboxed
untrusted code. Packages may not contain install scripts or native addons.
