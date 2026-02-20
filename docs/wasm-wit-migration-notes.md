# WIT Migration Notes (Legacy Plugin API -> Wasm Component API)

This document maps major concepts from the current dynamic plugin API to the
new WIT package in `wit/stellatune-plugin/`.

## Capability Mapping

- `Decoder` (`StCapabilityKind::Decoder`)
  - old: decoder instance vtable
  - new: `interface decoder` (`capabilities.wit`)
- `SourceCatalog` (`StCapabilityKind::SourceCatalog`)
  - old: source catalog instance + open stream IO handle
  - new: `interface source` with `resource catalog` and `resource stream`
- `LyricsProvider` (`StCapabilityKind::LyricsProvider`)
  - old: async JSON search/fetch
  - new: `interface lyrics` with `resource provider`
- `OutputSink` (`StCapabilityKind::OutputSink`)
  - old: sink vtable with negotiate/open/write/reset/close
  - new: `interface output-sink` (`output-sink.wit`)
- `Dsp` (`StCapabilityKind::Dsp`)
  - old: DSP vtable
  - new: `interface dsp` (`dsp.wit`)

## Lifecycle Mapping

- old:
  - module-level `begin_quiesce`
  - module-level `begin_shutdown`
- new:
  - `lifecycle.on-enable()`
  - `lifecycle.on-disable(reason)`

Host policy:

- Call `on-enable` after activation and before capability calls.
- Call `on-disable` before unload/disable/shutdown/reload.
- Force teardown on disable timeout/failure.

## Decoder Mapping

- old `StDecoderInfo` fields:
  - sample rate / channels
  - duration flag + value
  - seekable flag
  - gapless fields (`encoder_delay_frames`, `encoder_padding_frames`)
- new:
  - `common.decoder-info`
  - returned by `decoder.session.info()`

Gapless trim should use:

- `decoder-info.encoder-delay-frames`
- `decoder-info.encoder-padding-frames`

## Source Catalog Mapping

- old:
  - `begin_list_items_json_utf8`
  - `begin_open_stream`
  - stream via `StIoVTable`
- new:
  - `source.catalog.list-items-json(request-json)`
  - `source.catalog.open-stream-json(track-json) -> source.stream`
  - `source.stream.read(...)`

`source.catalog.open-uri(uri)` is provided as a convenience path for URI-driven
plugins.

## Config Update and State Transfer

Old API had per-capability plan/apply/export/import variants. New API unifies
shape using shared types:

- `common.config-update-mode`
- `common.config-update-plan`

Resources supporting update/state:

- `decoder.session`
- `source.catalog`
- `lyrics.provider`
- `output-sink.session`
- `dsp.processor`

Methods:

- `plan-config-update-json(new-config-json)`
- `apply-config-update-json(new-config-json)`
- `export-state-json()`
- `import-state-json(state-json)`

## Sidecar Mapping

- old plugin SDK:
  - spawn sidecar via process helpers under runtime root
- new:
  - host import `sidecar` in `host-imports.wit`
  - `launch`, `open-control`, `open-data`, transport introspection
  - host-managed lifecycle with plugin hook integration

## Hot-Path (Sink/DSP First)

Hot-path negotiation is now explicit for realtime-heavy capabilities:

- `output-sink.session.describe-hot-path(spec)`
- `dsp.processor.describe-hot-path(spec)`

Return contract:

- `some(hot-path.core-module-spec)`: host may instantiate the referenced core
  Wasm module and run pointer/length ABI for near-zero-copy processing.
- `none`: host falls back to regular component calls.

Core ABI details for `some(spec)` path are defined in:

- `docs/hot-path-core-abi.md`

Current rollout priority:

- first: `output-sink` and `dsp`
- later (optional): `decoder`

## World Selection

Choose world by ability + sidecar requirement:

- decoder: `decoder-plugin` / `decoder-plugin-sidecar`
- source: `source-plugin` / `source-plugin-sidecar`
- lyrics: `lyrics-plugin` / `lyrics-plugin-sidecar`
- output sink: `output-sink-plugin` / `output-sink-plugin-sidecar`
- dsp: `dsp-plugin` / `dsp-plugin-sidecar`
