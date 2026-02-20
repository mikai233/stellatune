# Wasm Plugin SDK Quickstart (Phase 5)

This guide covers a minimal workflow with the new SDK-only plugin path.

## 1. Generate a Plugin Scaffold

Use the scaffold script from repository root:

```powershell
tools/wasm-plugin-sdk/new-plugin.ps1 `
  -Name "Example Lyrics" `
  -PluginId "dev.stellatune.example.lyrics" `
  -Ability "lyrics" `
  -TypeId "example-lyrics"
```

The script creates:

- `sandbox-wasm-plugins/<crate-name>/Cargo.toml`
- `sandbox-wasm-plugins/<crate-name>/src/lib.rs`
- `sandbox-wasm-plugins/<crate-name>/plugin.json`

## 2. Build the Component

```powershell
cd sandbox-wasm-plugins/<crate-name>
cargo build --release --target wasm32-wasip2
```

## 3. Package the Artifact

From repository root:

```powershell
tools/wasm-plugin-sdk/package-plugin.ps1 `
  -ProjectDir sandbox-wasm-plugins/<crate-name>
```

Output layout:

- `target/plugins/<plugin-id>/plugin.json`
- `target/plugins/<plugin-id>/wasm/*.wasm`
- `target/plugins/<plugin-id>-<version>.zip`

## 4. Install with Wasm Runtime Package API

Use `crates/stellatune-wasm-plugins` package functions against the generated
plugin root directory.

## Notes

- This flow is intentionally Wasm-only; legacy dynamic plugin formats are out of
  scope.
- Generated code uses stub implementations (`unimplemented!` for complex
  methods). Replace those with real logic before packaging.
