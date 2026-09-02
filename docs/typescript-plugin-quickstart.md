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
ui/                      # optional static UI
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

Installing, updating, enabling, disabling, or uninstalling a plugin pauses
playback, tears down the old session, commits the package change, rebuilds the
registry and pipeline, and resumes from the actual consumed position when
appropriate.

## Trust model

Manifest v2 intentionally has no generic permissions list. Installed code is
treated as first-party or explicitly trusted local code, not as sandboxed
untrusted code. Packages may not contain install scripts or native addons.
