# stellatune-backend-api

This crate is the UI-agnostic backend entry for StellaTune.
It contains no Flutter/FRB binding code and can be used directly by Rust frontends such as TUI/CLI.

## Goals

- Keep backend domain logic independent from UI adapters.
- Provide one clear app/session entry for non-Flutter frontends.
- Keep FRB-specific translation in `stellatune-ffi` only.

## Module Layout

- `app`: top-level backend facade (`BackendApp`)
- `session`: runtime session assembly (`BackendSession`, options)
- `player`: player service (`PlayerService`) and plugin package management helpers
- `library`: library service (`LibraryService`)
- `runtime`: plugin runtime router/event hub/shared plugin runtime state

## Recommended Usage

### 1. Create app + session

```rust
use anyhow::Result;
use stellatune_backend_api::app::BackendApp;
use stellatune_backend_api::session::BackendSessionOptions;

fn main() -> Result<()> {
    let app = BackendApp::new();

    let mut session = app.create_session(BackendSessionOptions::with_library(
        "./data/library.db",
    ))?;

    session.player().play();

    if let Some(library) = session.library() {
        library.scan_all();
    }

    Ok(())
}
```

### 2. Subscribe plugin runtime events

```rust
use anyhow::Result;
use stellatune_backend_api::runtime::subscribe_plugin_runtime_events_global;

fn main() -> Result<()> {
    let rx = subscribe_plugin_runtime_events_global();
    for ev in rx.iter() {
        println!("plugin={} kind={:?}", ev.plugin_id, ev.kind);
    }

    Ok(())
}
```

### 3. Manage plugin packages

```rust
use anyhow::Result;
use stellatune_backend_api::app::BackendApp;

fn main() -> Result<()> {
    let app = BackendApp::new();

    let id = app.plugins_install_from_file(
        "./plugins".to_string(),
        "./downloads/demo-plugin.zip".to_string(),
    )?;

    println!("installed: {id}");
    Ok(())
}
```

## Notes

- This crate does not use `pub use` re-export shortcuts. Import symbols from explicit modules, e.g. `stellatune_backend_api::session::BackendSession`.
- `stellatune-ffi` is now an adapter crate that calls into this crate.
- If you need a new frontend (TUI/CLI), build it directly on top of `app` + `session`.
