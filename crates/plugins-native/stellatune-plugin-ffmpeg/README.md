# stellatune-plugin-ffmpeg

FFmpeg sidecar plugin scaffold for StellaTune.

Current status:
- `M1/M2` implemented: plugin package layout, decoder/encoder components, sidecar binary probe.
- `M3` implemented: buffered decode/encode pipeline via ffmpeg sidecar (`DecoderSession` + `EncoderSession` usable for local transcode).
- Current mode is one-shot buffered processing (not low-latency streaming).

## Build

```powershell
cargo build --manifest-path crates/plugins-native/stellatune-plugin-ffmpeg/decoder/Cargo.toml --release --target wasm32-wasip2
cargo build --manifest-path crates/plugins-native/stellatune-plugin-ffmpeg/encoder/Cargo.toml --release --target wasm32-wasip2
```

## Package (Windows)

Default packaging auto-downloads `ffmpeg.exe` and `ffprobe.exe` and bundles them into plugin `bin/`:

```powershell
.\crates\plugins-native\stellatune-plugin-ffmpeg\scripts\package-windows.ps1 -Configuration Release
```

Offline or pinned-local packaging can pass explicit binary paths:

```powershell
.\crates\plugins-native\stellatune-plugin-ffmpeg\scripts\package-windows.ps1 `
  -Configuration Release `
  -FfmpegExePath C:\tools\ffmpeg\bin\ffmpeg.exe `
  -FfprobeExePath C:\tools\ffmpeg\bin\ffprobe.exe `
  -SkipFfmpegDownload
```

## Package layout

The plugin package should contain:

- `plugin.json`
- `wasm/stellatune_plugin_ffmpeg_decoder.wasm`
- `wasm/stellatune_plugin_ffmpeg_encoder.wasm`
- `bin/ffmpeg(.exe)`
- `bin/ffprobe(.exe)`

The default config uses:
- `ffmpeg_path = "bin/ffmpeg"`
- `ffprobe_path = "bin/ffprobe"`

## Notes

- The host resolves relative sidecar executables under plugin root and `bin/`.
- Windows `.exe` suffix is automatically resolved by host sidecar path logic.
- Encoder `options_json` supports:
  - `["-q:a","2", ...]`
  - `{"ffmpeg_args":["-q:a","2", ...]}`
