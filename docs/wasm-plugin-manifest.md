# Wasm Plugin Manifest (v1, Frozen 2026-02-21)

This document defines the current manifest contract for Wasm plugins.

## Goals

- Keep install/uninstall as one logical plugin package.
- Allow one plugin package to contain multiple Wasm components.
- Keep capability routing explicit by `(kind, type_id)`.
- Keep capability metadata in manifest for host UI/runtime selection.

## Freeze Notes

- This is the canonical contract for `schema_version: 1`.
- Plugin packages targeting this runtime must emit `schema_version: 1`.
- Additive optional fields are allowed without schema bump.
- Breaking manifest shape changes require a new `schema_version`.

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
          "type_id": "ncm",
          "display_name": "NCM Decoder",
          "config_schema_json": "{}",
          "default_config_json": "{}",
          "decoder": {
            "ext_scores": [
              {
                "ext": "ncm",
                "score": 100
              }
            ],
            "wildcard_score": 0
          }
        }
      ]
    },
    {
      "id": "source-netease",
      "path": "wasm/source_netease.wasm",
      "world": "stellatune:plugin/source-plugin@0.1.0",
      "abilities": [
        {
          "kind": "source",
          "type_id": "netease",
          "display_name": "NetEase Source",
          "config_schema_json": "{}",
          "default_config_json": "{}"
        }
      ]
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

Ability fields:

- `kind`: `decoder | source | lyrics | output-sink | dsp`.
- `type_id`: Existing type id concept used by capability routing.
- `display_name` (optional): UI-facing name.
- `config_schema_json` (optional): JSON schema string for config editing.
- `default_config_json` (optional): default config JSON string.
- `decoder` (required when `kind=decoder`):
  - `ext_scores`: exact extension score rules.
  - `wildcard_score`: fallback score for unmatched extension.

## Validation Rules

- `id`, `name`, `version` must be non-empty.
- each component `path` must be safe relative path under plugin root.
- each component `world` must be non-empty.
- `(kind, type_id)` cannot collide within the same plugin package.
- `display_name` cannot be empty string when provided.
- `config_schema_json` and `default_config_json` must be valid JSON when provided.
- decoder abilities must provide decoder rules.
- decoder rules must not contain empty/`*` ext entries and must not duplicate extensions.

## Runtime Rules

- Host validates every component `world` before activation.
- Host binds exported ability to `(plugin_id, kind, type_id, component_id)`.
- Host owns sidecar lifecycle for sidecar-capable worlds.
- Host should prefer hot-path core modules for `output-sink` and `dsp` when
  `describe-hot-path(...)` returns `some(spec)`.
- If `describe-hot-path(...)` returns `none`, host uses normal component calls.
- Host must call lifecycle hooks on component state transitions:
  - call `lifecycle.on-enable()` immediately after component activation and before first capability call.
  - call `lifecycle.on-disable(reason)` before component unload/disable.
  - reasons: `host-disable | unload | shutdown | reload`.

## Migration Notes

Runtime receipt now stores the manifest snapshot for discovery and reload checks:

- keep top-level plugin install root per `id`
- persist original `plugin.json`
- persist install receipt `.install.json` with manifest payload
