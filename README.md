# StellaTune (星律)

StellaTune is an open-source music player built with **Flutter** (UI) and **Rust** (core).
The project focuses on a modern local-library UX plus a Rust-first playback runtime with plugin extensibility.

> **Status:** Early development (WIP)

## What Works Now

- Desktop Flutter app skeleton with Rust backend integration.
- Rust audio pipeline with plugin-capable decoder/source stages.
- Native plugin crates for local decoding and NetEase-related source/decoder flows.
- Wasm plugin runtime and host bindings under active development.

Current playback constraints:
- MVP path is centered on local playback and plugin integration.
- Stereo (2-channel) output is currently the primary tested path.
- Some advanced features (gapless, DSP UX, scripting automation) are still planned.

## Quick Start (Developers)

### Prerequisites

- Flutter SDK (stable channel recommended)
- Rust toolchain (stable)
- `flutter_rust_bridge_codegen`
- Node.js 20 (needed for NetEase sidecar packaging/smoke path)

Install FRB codegen:

```bash
cargo install flutter_rust_bridge_codegen --locked
```

If you build Wasm plugins locally:

```bash
rustup target add wasm32-wasip2
```

### Run Desktop App (Windows example)

```bash
cd apps/stellatune
flutter pub get
flutter_rust_bridge_codegen generate
flutter run -d windows
```

Notes:
- Desktop runners auto-build Rust artifacts during `flutter run` / `flutter build`.
- The repo intentionally does not use `flutter_rust_bridge_codegen integrate`; FRB is used for bindings/codegen only.

## Monorepo Layout

- `apps/stellatune`: Main Flutter desktop app.
- `apps/stellatune-tui`: Rust TUI app target.
- `crates/stellatune-audio*`: Core audio runtime and adapters.
- `crates/stellatune-plugins`: Host-side plugin runtime and stream host services.
- `crates/stellatune-plugin-sdk`: SDK for plugin implementations.
- `crates/plugins-native`: Native/plugin crates (ASIO, NCM, NetEase source/decoder, etc.).
- `tools/stellatune-ncm-sidecar`: NetEase sidecar service used by plugin flows.
- `wit`: WIT interfaces for component/plugin boundaries.
- `docs`: Architecture, plugin protocol, and migration notes.

## Plugin Architecture (Snapshot)

- Plugin categories are modeled around worlds such as `source`, `decoder`, `lyrics`, `dsp`, and `output-sink`.
- Plugin manifests are JSON-based (`plugin.json` in each plugin crate).
- NetEase plugin currently consists of two Wasm crates: `crates/plugins-native/stellatune-plugin-netease/source` and `crates/plugins-native/stellatune-plugin-netease/decoder`.
- Windows packaging script for NetEase plugin:

```powershell
powershell -ExecutionPolicy Bypass -File crates/plugins-native/stellatune-plugin-netease/scripts/package-windows.ps1
```

Useful docs:
- `docs/wasm-plugin-sdk-quickstart.md`
- `docs/wasm-plugin-manifest.md`
- `docs/plugin-event-protocol.md`

## CI Checks

GitHub Actions currently runs:

- Flutter CI (`.github/workflows/flutter.yml`)
- `flutter analyze`
- `flutter build windows --debug`
- Rust CI (`.github/workflows/rust.yml`)
- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- NetEase plugin Windows smoke: build Wasm source/decoder crates, package plugin artifact, verify packaged wasm + sidecar files, and run sidecar `/health` smoke check.

## Recommended Local Checks Before PR

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
```

For Flutter app changes:

```bash
cd apps/stellatune
flutter analyze
flutter build windows --debug
```

## Contributing

- Use small, focused PRs.
- Prefer Conventional Commits for commit messages.
- Keep plugin schema/config changes synchronized with code, `plugin.json`, and plugin README docs.
- If you touch CI-sensitive paths, run the local checks above first.

## Troubleshooting

- `target wasm32-wasip2 not found`: run `rustup target add wasm32-wasip2`.
- `flutter_rust_bridge_codegen: command not found`: install with `cargo install flutter_rust_bridge_codegen --locked`.
- NetEase packaging fails at `npm` steps: ensure Node.js 20 is installed and run in `tools/stellatune-ncm-sidecar`.
