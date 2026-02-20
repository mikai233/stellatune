# Wasm Plugin Manifest (v1, Frozen 2026-02-20)

This draft defines how one installed plugin package can contain multiple Wasm
components, each implementing one capability ("ability") with independent
runtime/thread policy.

## Goals

- Keep install/uninstall as one logical plugin package.
- Allow each ability to compile as a separate Wasm component.
- Allow host to run each ability in a different worker thread.
- Keep metadata typed at interface level (see `wit/stellatune-plugin/*.wit`).

## Freeze Notes

- This document is the canonical manifest contract for schema version `1`.
- Plugin packages targeting this runtime must emit `schema_version: 1`.
- Field additions are allowed only as backward-compatible optional fields.
- Any breaking manifest shape change must bump `schema_version`.

## Package Layout Example

```text
plugin-com.netease/
  plugin.json
  wasm/
    decoder_ncm.wasm
    source_netease.wasm
    lyrics_netease.wasm
  sidecar/
    stellatune-ncm-sidecar.exe
```

WIT package layout (recommended):

```text
wit/stellatune-plugin/
  common.wit
  host-imports.wit
  capabilities.wit
  hot-path.wit
  output-sink.wit
  dsp.wit
  worlds.wit
```

## Manifest Shape

```json
{
  "schema_version": 1,
  "id": "com.netease",
  "name": "NetEase",
  "version": "1.2.0",
  "api_version": 1,
  "components": [
    {
      "id": "decoder-ncm",
      "path": "wasm/decoder_ncm.wasm",
      "world": "stellatune:plugin/decoder-plugin@0.1.0",
      "abilities": [
        {
          "kind": "decoder",
          "type_id": "ncm"
        }
      ],
      "threading": {
        "model": "dedicated",
        "max_instances": 2
      }
    },
    {
      "id": "source-netease",
      "path": "wasm/source_netease.wasm",
      "world": "stellatune:plugin/source-plugin@0.1.0",
      "abilities": [
        {
          "kind": "source",
          "type_id": "netease"
        }
      ],
      "threading": {
        "model": "dedicated",
        "max_instances": 4
      }
    },
    {
      "id": "lyrics-netease",
      "path": "wasm/lyrics_netease.wasm",
      "world": "stellatune:plugin/lyrics-plugin@0.1.0",
      "abilities": [
        {
          "kind": "lyrics",
          "type_id": "netease-lyrics"
        }
      ],
      "threading": {
        "model": "shared_pool",
        "pool": "io"
      }
    }
  ]
}
```

## Field Definitions

- `schema_version`: Manifest schema revision.
- `id`: Logical plugin id. Install lifecycle is keyed by this id.
- `name`: Human-friendly name.
- `version`: Plugin package version.
- `api_version`: Host-plugin contract version for manifest/runtime policy.
- `components`: List of Wasm components in this package.

Component fields:

- `id`: Stable component id within the package.
- `path`: Relative path to component `.wasm`.
- `world`: Expected component world string.
- `abilities`: One or more abilities provided by this component.
- `threading`: Host scheduling hint for this component.

Hot-path fields:

- Hot-path core module artifacts are optional.
- When present, they are declared by the plugin at runtime through:
  - `output-sink.session.describe-hot-path(...)`
  - `dsp.processor.describe-hot-path(...)`
- `hot-path.core-module-spec.wasm-rel-path` is resolved relative to plugin root.

Ability fields:

- `kind`: `decoder | source | lyrics | output_sink | dsp`.
- `type_id`: Existing type id concept used by capability routing.

Threading fields:

- `model`: `dedicated | shared_pool`.
- `max_instances`: Optional hard cap for dedicated workers.
- `pool`: Optional pool name when `model=shared_pool`.

## Runtime Rules

- Host validates every component `world` before activation.
- Host binds exported ability to `(plugin_id, kind, type_id, component_id)`.
- Host may instantiate different component ids in different threads.
- Host owns sidecar lifecycle for sidecar-capable worlds.
- Host should prefer hot-path core modules for `output-sink` and `dsp` when
  `describe-hot-path(...)` returns `some(spec)`.
- If `describe-hot-path(...)` returns `none`, host uses normal component calls.
- Core ABI contract for this path is documented in
  `docs/hot-path-core-abi.md`.
- Host must call lifecycle hooks on component state transitions:
  - call `lifecycle.on-enable()` immediately after component activation and
    before first capability call.
  - call `lifecycle.on-disable(reason)` before component unload/disable.
  - reasons: `host-disable | unload | shutdown | reload`.
- Sidecar-oriented components should launch required sidecars in `on-enable()`
  and stop/terminate them in `on-disable(...)`.
- If `on-disable(...)` times out or fails, host still proceeds with forced
  teardown and process cleanup.
- On plugin unload:
  - stop ability instances
  - call `lifecycle.on-disable(unload)` for each loaded component
  - terminate component-scoped sidecars
  - release Wasm instances

## Metadata Policy

Typed metadata should be transferred with the records in
`wit/stellatune-plugin/common.wit`:

- `common.audio-tags`
- `common.encoded-audio-format`
- `common.media-metadata`

Use `media-metadata.extras` only for plugin-specific fields not covered by
common records.

## Migration Notes (Current Native Runtime)

Current runtime receipt uses a single `library_rel_path`. For Wasm packaging,
evolve receipt/manifest storage to preserve all component paths:

- keep top-level plugin install root per `id`
- persist original `plugin.json`
- persist normalized component table for runtime discovery

This allows old native plugins and new Wasm multi-component plugins to coexist
during migration.
