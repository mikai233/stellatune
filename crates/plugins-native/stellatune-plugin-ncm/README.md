# NCM local-file plugin

Optional Manifest v2 plugin for local .ncm files. Install its platform-specific
ZIP and enable it before opening or scanning NCM files. The APP does not bundle
the plugin or link ncmdump; that dependency exists only in stellatune-ncm-host.

plugin.mjs is a small Node RPC adapter. The plugin's Rust executable reuses the
former ncmdump reader, reads metadata, and serves decrypted MP3/FLAC byte ranges
on 127.0.0.1 at a random port. Symphonia in the host decodes those bytes into PCM.
No audio bytes pass through JSON RPC. No decrypted files or whole-track memory
caches are produced. Each HTTP response uses bounded 64 KiB chunks and a two-slot
queue. Container probing scans at most 1 MiB for the FLAC header. The host's HTTP
source retains at most 2 MiB of encoded chunks to let seek retries make progress.

Source files are read-only. Library and queue identities retain their original
paths. resolve-file reuses URLs for unchanged files within a process. Each GET or
Range request owns an independent reader. HEAD, suffix/open/bounded ranges and
416 are supported. URLs from an old process are not persistent identities.

Processes start on first use and remain resident. Disable, update, uninstall or
APP exit closes the plugin and its HTTP server; ongoing streams then fail rather
than silently retaining decrypted content. If Node exits unexpectedly, the Rust
host exits on stdin EOF. A later resolution restarts a crashed Rust host. There
is no automatic recovery of previously issued URLs across process lifetimes.
Legacy plugin-data/dev.stellatune.source.ncm/cache files are ignored and are not
automatically deleted. They can be removed manually while the old plugin is stopped.

## Build and test on Windows

From the workspace root:

~~~powershell
powershell -ExecutionPolicy Bypass -File crates/plugins-native/stellatune-plugin-ncm/scripts/package-windows.ps1
node --test crates/plugins-native/stellatune-plugin-ncm/tests/plugin.test.mjs
cargo test -p stellatune-backend-api local_plugin
~~~

The ZIP contains manifest.json, plugin.mjs and bin/stellatune-ncm-host.exe.
No npm dependencies, native addon or WebUI is required. For another platform,
build host/Cargo.toml for that platform and place the executable (without .exe
on Unix) in bin/, preserving executable permissions when packaging.

Tests use synthetic FLAC/MP3 tones. Fixture regeneration uses
node tests/fixtures/generate.mjs [mp3] and requires ffmpeg only for generation.
