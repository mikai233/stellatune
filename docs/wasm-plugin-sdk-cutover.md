# Wasm Plugin SDK Cutover Policy (Frozen 2026-02-20)

## Decision

Stellatune plugin development moves to the Wasm component model only.

The canonical contracts are:

- `wit/stellatune-plugin/*.wit`
- `docs/wasm-plugin-manifest.md` (schema version `1`)
- `docs/hot-path-core-abi.md` (`abi_version = 1`)

## Scope

- New plugin authoring targets the new crate `stellatune-wasm-plugin-sdk`.
- Runtime loading targets `crates/stellatune-wasm-plugins`.
- Plugin package format is `plugin.json` + one or more Wasm components.

## Non-Goals

- No compatibility adapter for the legacy dynamic-library plugin runtime.
- No mixed ABI plugin authoring surface in a single SDK.
- No legacy package installation fallback.

## Enforcement

- Legacy plugin artifacts are rejected at install time with actionable errors.
- New backend/runtime integration uses Wasm plugin paths only after cutover.
- API and docs changes that break this contract require explicit version bumps.
