# StellaTune

English | [简体中文](README_zh.md)

> A next-generation, cross-platform music player designed for audiophiles and tinkerers alike.
>
> Status: Early stage / WIP. StellaTune is under active development, and APIs, plugin interfaces, and user-facing features may change frequently.
> Platform support priority currently focuses on Windows first, then the rest of the desktop platforms, and mobile platforms last.

StellaTune is not just another music player; it is an open-source, extensible audio platform built with a beautiful **Flutter** user interface and a blazing-fast, memory-safe **Rust** core. Whether you are managing a massive local library or streaming through custom plugins, StellaTune delivers an uncompromising audio experience.

---

## Key Features

- **Audiophile Grade Audio**: Pursuing true high-fidelity and low-latency playback. The Rust-first audio pipeline
  ensures that your music is delivered exactly as it was meant to be heard, without compromise.
- **Cross-Platform Excellence**: Built from the ground up to be truly cross-platform. Enjoy a fluid, responsive, and
  native-feeling UI across desktop (Windows, macOS, Linux) and mobile platforms, all unified under a robust Rust core.
- **Powerful Plugin Ecosystem**: Limitless extensibility. StellaTune features a highly versatile WebAssembly (Wasm) and
  Native plugin system. Tailor the player to your exact needs by dropping in bespoke audio sources, decoders, lyrics
  providers, DSPs, and output sinks.
- **Modern Local Library**: A carefully crafted, aesthetically pleasing user experience designed for the modern music
  lover to effortlessly build, organize, and enjoy their local collection.

## Screenshots

![StellaTune Player Interface](docs/assets/app-screenshot.png)

---

## Getting Started

### For Users
*(Pre-compiled binaries and installers for Windows, macOS, and Linux will be available in the Releases page soon.)*

### For Developers

StellaTune welcomes developers to build upon its core foundation or create amazing new plugins for the community.

#### Prerequisites

To build StellaTune from source, you will need:
- [Flutter SDK](https://flutter.dev/docs/get-started/install) (stable channel recommended)
- [Rust toolchain](https://rustup.rs/) (stable)
- [Node.js 20](https://nodejs.org/) (needed for specific plugin packaging and sidecar services)

Prepare your environment by installing the code generator and adding the Wasm target:

```bash
cargo install flutter_rust_bridge_codegen --locked
rustup target add wasm32-wasip2
```

#### Running the Desktop App (Example: Windows)

```bash
cd apps/stellatune
flutter pub get
flutter_rust_bridge_codegen generate
flutter run -d windows
```
*Note: The Rust backend artifacts are automatically built during the `flutter run` or `flutter build` process.*

---

## Plugin Development

StellaTune's true power lies in its modular architecture. Plugins can be written in Rust (and other languages compiling to Wasm), and loaded dynamically at runtime. The plugin worlds include: `source`, `decoder`, `lyrics`, `dsp`, and `output-sink`.

Want to build your own extension? Check out our technical guides:
- [Wasm Plugin SDK Quickstart](docs/wasm-plugin-sdk-quickstart.md)
- [Wasm Plugin Manifest Guide](docs/wasm-plugin-manifest.md)
- [Plugin Event Protocol](docs/plugin-event-protocol.md)

---

## Architecture & Monorepo

The StellaTune monorepo is thoughtfully structured to separate concerns while making development straightforward:

- **`apps/stellatune`**: The main user-facing application (Flutter desktop/mobile).
- **`apps/stellatune-tui`**: A terminal user interface (Rust TUI) leveraging the same core.
- **`crates/stellatune-audio*`**: The core audio runtime, pipeline, and playback adapters.
- **`crates/stellatune-plugins`**: The host-side plugin runtime and service coordinators.
- **`crates/stellatune-plugin-sdk`**: The SDK for implementing your own plugins.
- **`crates/plugins-native`**: First-party Native and Wasm plugins (e.g., ASIO output, NetEase integration).
- **`tools/*`**: Utility services like the NetEase sidecar used by plugin flows.

---

## Contributing

We welcome contributions of all sizes! Whether it's reporting bugs, discussing new features, or submitting code, your help is appreciated.

- **Small, focused PRs** are the best way to get your code merged quickly.
- Please use **Conventional Commits** for your commit messages.
- If you're modifying CI-sensitive code, ensure you run local checks before pushing:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
```

For Flutter UI changes:
```bash
cd apps/stellatune
flutter analyze
flutter build windows --debug
```

## License

[MIT License](LICENSE) (or see individual crate licenses where applicable).
