# Bundled plugin runtime

Desktop builds ship Node **24.20.0** and the shared JavaScript runner. Users do
not need Node, npm, or a source checkout. One executable is shared on disk;
each active plugin has its own lazily started process.

Windows/Linux Flutter CMake builds prepare and install `plugin-runtime/` beside
the application executable. macOS's `Bundle Plugin Runtime` Xcode phase installs
both `node-arm64` and `node-x64` in `Contents/MacOS/plugin-runtime/` and signs them
when signing is enabled. The macOS build machine needs `cmake` on PATH (Homebrew
locations are included). Runtime sandbox permissions still come from the macOS
application; bundling Node does not grant new filesystem/network access.

`prepare.cmake` pins the official release archives and their SHA-256 checksums.
It downloads at **build time**, caches under `target/node-runtime-cache/`, and
copies only Node, its license/version, and the runner/helper scripts into the
application. npm and development dependencies are not shipped. Once the matching
archive is cached, rebuilding works offline; a checksum mismatch fails the build.
Update the version and all archive hashes together when upgrading Node.

The backend uses absolute paths relative to the application executable. Missing
runtime files cause a plugin startup error, never a fallback to system Node or
the repository. For explicit development overrides, set `STELLATUNE_NODE_BINARY`
and/or `STELLATUNE_TYPESCRIPT_RUNNER` to absolute paths before starting the APP.
Rust-only plugin tests can still supply their own Node through the runtime API.

On Windows, the Rust process launcher sets `CREATE_NO_WINDOW`. JS plugins that
spawn their own console sidecars must also set `windowsHide: true`; the first-party
NCM and Netease plugins already do this. Piped stdout/stderr remain available for
RPC and diagnostics.

Build and verify on Windows:

```powershell
cd apps/stellatune
flutter build windows --release
cd ../..
$env:STELLATUNE_TEST_RUNTIME_DIR = "$PWD/apps/stellatune/build/windows/x64/runner/Release/plugin-runtime"
& "$env:STELLATUNE_TEST_RUNTIME_DIR/node.exe" --test tools/typescript-plugin-runtime/tests/bundle.test.mjs
```

The test relocates the packaged runtime to a temporary directory containing
spaces and clears the child process PATH. It verifies RPC, the bundled Node
version/path, helper imports, HTTP, and shutdown. Windows CI runs it against the
Flutter build output. Linux/macOS packaging requires validation on those systems.
