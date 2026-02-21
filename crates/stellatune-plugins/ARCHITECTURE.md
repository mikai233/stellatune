# stellatune-wasm-plugins Architecture

This crate is designed as a standalone Wasm plugin runtime foundation.
It is intentionally decoupled from the legacy native plugin runtime.

## Module Layout

- `src/manifest.rs`
  - Manifest schema and validation (`plugin.json`).
  - Install receipt and uninstall pending marker format.
  - Installed plugin discovery and pending uninstall cleanup.

- `src/package/mod.rs`
  - Package lifecycle operations:
    - `install_from_artifact(...)`
    - `list_installed(...)`
    - `uninstall_by_id(...)`
  - Artifact handling (directory/zip), path safety, install root layout.

- `src/host/`
  - Host-side contracts used by runtime/executor.
  - `lifecycle.rs`: lifecycle host trait (`activate/deactivate/shutdown`).

- `src/executor/`
  - Wasm execution boundary contract.
  - `mod.rs`: executor trait, `WasmtimeExecutor`, and execution-focused tests.

- `src/runtime/`
  - Runtime state and orchestration.
  - `model.rs`: runtime DTOs and state enums.
  - `registry.rs`: internal registry model and manifest-to-runtime mapping.
  - `service.rs`: `WasmPluginRuntime` orchestration API.

## Runtime Design

`WasmPluginRuntime` is an explicit instance (no global singleton required).

Core behavior:
- Maintains desired state (`Enabled`/`Disabled`) and active runtime state.
- Syncs plugins from disk (`sync_plugins`) and emits structured change report.
- Routes lifecycle transitions through `RuntimeLifecycleHost`.
- Provides capability index lookup by `(plugin_id, kind, type_id)`.
- Can be constructed with:
  - `WasmPluginRuntime::with_noop_host()` for pure registry/testing flow.
  - `WasmPluginRuntime::with_executor(...)` to route lifecycle into executor.

## External Integration Contract

Integrators should depend on explicit modules:

- Package plane:
  - `crate::package::install_from_artifact`
  - `crate::package::list_installed`
  - `crate::package::uninstall_by_id`

- Runtime plane:
  - `crate::runtime::service::WasmPluginRuntime`
  - `crate::host::lifecycle::RuntimeLifecycleHost`
  - `crate::runtime::model::*`

No compatibility wrappers are provided by design.

## Current Executor Scope

`WasmtimeExecutor` currently instantiates component files for worlds that do not
require host imports (e.g. non-sidecar/no-host-import worlds). Worlds requiring
`host-stream`, `http-client`, or `sidecar` imports are rejected until host
import wiring is provided.
