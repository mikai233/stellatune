# ASIO Output

Windows x64 output plugin for Stellatune. Install the ZIP in Settings > Plugins,
then choose **ASIO** under Settings > Audio and select your driver (for example
SMSL's **USB DAC ASIO**). The driver's default sample rate and hardware buffer
are used unless explicitly configured. Enable **Match track sample rate** in
Settings > Audio to use the track's rate when supported; otherwise the core
resamples to the driver's default. An explicit plugin `sample_rate` overrides
this toggle. Current output layouts are mono/stereo.

The package contains `stellatune-asio-host.exe`. The host loads the ASIO driver;
the Rust audio adapter sends interleaved float PCM over an SPSC memory mapping.
The mapping is backed by one temporary file per output session, never explicitly
flushed, and deleted when the session closes. It is not a decoded music cache.
Binary stdin/stdout RPC carries controls and clock snapshots only. Playback does
not start Node. Host creation hides the Windows console window.

Pause retains accepted audio and keeps driver callbacks writing silence. It does
not call CPAL's ASIO pause, which skips callbacks while leaving old hardware
buffers intact. Resume continues with the retained audio. Seek resets both the mapping reader and driver
queue before establishing a new playback clock epoch. The low/medium/high
latency presets bound combined queued PCM by time at the negotiated sample
rate. The actual hardware callback size imposes a minimum capacity.

Stop, switching to a system output, disabling, updating, uninstalling, and app
shutdown release the ASIO host. Package changes fall back to the system output;
select ASIO again afterwards. A crashed or unresponsive host fails with a bounded
timeout and can be opened again. Device IDs remain stable across host restarts.

Build from the repository root (Rust, MSVC tools and clang/bindgen required):

```powershell
./crates/plugins-native/stellatune-plugin-asio/scripts/package-windows.ps1 -AsioSdkDir C:/SDK/ASIOSDK
```

The ZIP is standalone and needs neither Node modules nor the source checkout.
Use it with a host supporting manifest `native_output.protocol = "asio-v9"`.
Old WASM ASIO packages are not compatible.

Installing this package over a schema-v1 ASIO installation replaces it with the
new runtime. The installer verifies the old `.install.json` and `plugin.json`
identity, and keeps the old directory as `.legacy-backup-dev.stellatune.output.asio-*`
under the plugins directory. Old settings are not imported; select ASIO and the
driver again after installation. Unrecognized same-name directories are left
untouched and must be moved aside before retrying.

The native host is GPL-3.0-only (see LICENSE.txt). Its source is in
[`stellatune-asio-host`](https://github.com/mikai233/stellatune/tree/master/crates/plugins-native/stellatune-asio-host).
The ASIO SDK has its own accompanying license, copied into `bin/licenses` when
building with an explicit SDK path. Host logs are written under
`%TEMP%/stellatune/asio-logs` (override with `STELLATUNE_ASIO_LOG_DIR`).
