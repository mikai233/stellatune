# `stellatune-backend-api`

`stellatune-backend-api` is the UI-agnostic backend facade for StellaTune.

It is designed for Rust frontends (CLI/TUI/desktop shells) that want to call
application services directly without going through Flutter/FRB bindings.

## Design Goals

- Keep app/domain orchestration independent from UI transport layers.
- Provide one async session entry for runtime services.
- Keep `stellatune-ffi` as an adapter layer, not as the domain owner.

## Current Module Layout

- `app`: high-level bootstrap facade (`BackendApp`).
- `session`: session assembly and service access (`BackendSession`, options).
- `library`: library domain service (`LibraryService`).
- `lyrics_service` + `lyrics_types`: lyrics orchestration and shared data models.
- `player`: plugin package install/list/uninstall helpers.
- `runtime`: shared runtime engine and plugin-runtime operations.

`lib.rs` currently re-exports lyrics model types:
- `LyricLine`
- `LyricsDoc`
- `LyricsEvent`
- `LyricsQuery`
- `LyricsSearchCandidate`

## Quick Start (Async)

```rust
use anyhow::Result;
use stellatune_backend_api::app::BackendApp;
use stellatune_backend_api::session::BackendSessionOptions;

#[tokio::main]
async fn main() -> Result<()> {
    let app = BackendApp::new();
    let session = app
        .create_session(BackendSessionOptions::with_library("./data/library.db"))
        .await?;

    session.player().play().await?;

    if let Some(library) = session.library() {
        library.scan_all().await?;
    }

    Ok(())
}
```

## Runtime Operations Exposed in This Crate

The `runtime` module is the integration surface for shared runtime state:

- output device routing:
  - `runtime_list_output_devices()`
  - `runtime_set_output_device(...)`
  - `runtime_output_sink_metrics()`
- output behavior:
  - `runtime_set_output_options(...)`
  - `runtime_set_output_sink_route(...)`
  - `runtime_clear_output_sink_route()`
- lifecycle:
  - `runtime_shutdown()`
- plugin runtime state:
  - `plugin_runtime_apply_state(...)`
  - `plugin_runtime_enable(...)`
  - `plugin_runtime_disable(...)`
  - `plugin_runtime_apply_state_status_json()`

## Plugin Package Management

`BackendApp` exposes sync helpers for plugin package files:

- `plugins_install_from_file(...)`
- `plugins_list_installed_json(...)`
- `plugins_uninstall_by_id(...)`

Use these for artifact-level package operations. Use `LibraryService` runtime
methods for enable/disable/apply-state behavior.

## Notes

- This crate intentionally does not expose Flutter-specific stream adapters.
- Legacy plugin host-event JSON bridge is no longer part of backend API usage.
- For non-Flutter frontends, build directly on top of `BackendApp` + `BackendSession`.
