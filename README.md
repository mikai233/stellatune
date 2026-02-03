# StellaTune(星律)

StellaTune is an open-source, cross-platform music player built with **Flutter** (UI) and **Rust** (core).

The project aims to combine a modern, fast library experience with a **Rust-first playback engine** that is designed to be low-latency, reliable under load, and extensible through plugins.

> **Status:** Early development (WIP)

---

## Goals

### 1) A modern library experience
- Fast browsing for large collections
- Instant search, filtering, and sorting
- Flexible queue management
- Clean, keyboard-friendly workflows (planned)
- Tag editing and batch operations (planned)

### 2) A Rust-first audio engine
- Decoding and playback pipeline implemented in Rust
- Audio-thread-safe design (no blocking IO, minimal locking)
- Gapless playback (planned)
- Crossfade (planned)
- Loudness normalization / ReplayGain-style gain control (planned)
- Resampling and device format adaptation (planned)
- EQ / DSP effect chain (planned)

### 3) Extensibility
StellaTune is designed to be “plugin-friendly”, inspired by foobar2000-style ecosystems.

Planned plugin categories:
- **Decoders / demuxers** (support more formats)
- **DSP effects** (EQ, limiter, convolution, etc.)
- **Output backends** (exclusive modes, alternative device APIs, etc.)

A lightweight scripting layer for automation (planned: Lua) may also be added for things like queue rules and event hooks.

---

## Development

### Prerequisites
- Flutter SDK
- Rust toolchain (stable)

### Run the Flutter app
```bash
cd apps/stellatune_app
flutter run
