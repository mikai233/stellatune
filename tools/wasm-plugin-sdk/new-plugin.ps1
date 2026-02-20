param(
  [Parameter(Mandatory = $true)]
  [string]$Name,
  [Parameter(Mandatory = $true)]
  [string]$PluginId,
  [Parameter(Mandatory = $true)]
  [ValidateSet("decoder", "source", "lyrics", "output-sink", "dsp")]
  [string]$Ability,
  [Parameter(Mandatory = $true)]
  [string]$TypeId,
  [string]$OutputDir = "sandbox-wasm-plugins",
  [string]$ComponentId = "",
  [string]$Version = "0.1.0"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if ([string]::IsNullOrWhiteSpace($ComponentId)) {
  $ComponentId = "$Ability-main"
}

$crateName = ($Name.ToLower() -replace "[^a-z0-9]+", "-").Trim("-")
if ([string]::IsNullOrWhiteSpace($crateName)) {
  throw "name produced an empty crate name"
}

$projectDir = Join-Path $OutputDir $crateName
if (Test-Path $projectDir) {
  throw "target directory already exists: $projectDir"
}

New-Item -ItemType Directory -Path $projectDir | Out-Null
New-Item -ItemType Directory -Path (Join-Path $projectDir "src") | Out-Null
New-Item -ItemType Directory -Path (Join-Path $projectDir "wasm") | Out-Null

switch ($Ability) {
  "decoder" {
    $world = "stellatune:plugin/decoder-plugin@0.1.0"
    $macroName = "export_decoder_plugin"
    $libCode = @"
use stellatune_wasm_plugin_sdk::prelude::*;

pub struct Plugin;
pub struct Session;

impl PluginLifecycle for Plugin {}
impl ConfigStateOps for Session {}

impl DecoderSession for Session {
    fn info(&self) -> SdkResult<DecoderInfo> {
        unimplemented!("fill decoder info")
    }

    fn metadata(&self) -> SdkResult<MediaMetadata> {
        unimplemented!("fill decoder metadata")
    }

    fn read_pcm_f32(&mut self, _max_frames: u32) -> SdkResult<PcmF32Chunk> {
        unimplemented!("decode PCM samples")
    }

    fn seek_ms(&mut self, _position_ms: u64) -> SdkResult<()> {
        unimplemented!("seek support")
    }
}

impl DecoderPlugin for Plugin {
    type Session = Session;
    const TYPE_ID: &'static str = "__TYPE_ID__";
    const DISPLAY_NAME: &'static str = "__NAME__";

    fn open(&mut self, _input: DecoderInput<'_>) -> SdkResult<Self::Session> {
        Ok(Session)
    }
}

fn create_plugin() -> SdkResult<Plugin> {
    Ok(Plugin)
}

stellatune_wasm_plugin_sdk::__MACRO__! {
    export: generated_export,
    plugin_type: crate::Plugin,
    create: crate::create_plugin,
    plugin_id: "__PLUGIN_ID__",
    component_id: "__COMPONENT_ID__",
    type_id: "__TYPE_ID__",
    display_name: "__NAME__",
}
"@
  }
  "source" {
    $world = "stellatune:plugin/source-plugin@0.1.0"
    $macroName = "export_source_plugin"
    $libCode = @"
use stellatune_wasm_plugin_sdk::prelude::*;

pub struct Plugin;
pub struct Catalog;
pub struct Stream;

impl PluginLifecycle for Plugin {}
impl ConfigStateOps for Catalog {}

impl SourceStream for Stream {
    fn metadata(&self) -> SdkResult<MediaMetadata> {
        unimplemented!("stream metadata")
    }

    fn read(&mut self, _max_bytes: u32) -> SdkResult<EncodedChunk> {
        unimplemented!("stream read")
    }
}

impl SourceCatalog for Catalog {
    type Stream = Stream;

    fn list_items_json(&mut self, _request_json: &str) -> SdkResult<String> {
        Ok("[]".to_string())
    }

    fn open_stream_json(&mut self, _track_json: &str) -> SdkResult<Self::Stream> {
        Ok(Stream)
    }
}

impl SourcePlugin for Plugin {
    type Catalog = Catalog;
    const TYPE_ID: &'static str = "__TYPE_ID__";
    const DISPLAY_NAME: &'static str = "__NAME__";

    fn create_catalog(&mut self) -> SdkResult<Self::Catalog> {
        Ok(Catalog)
    }
}

fn create_plugin() -> SdkResult<Plugin> {
    Ok(Plugin)
}

stellatune_wasm_plugin_sdk::__MACRO__! {
    export: generated_export,
    plugin_type: crate::Plugin,
    create: crate::create_plugin,
    plugin_id: "__PLUGIN_ID__",
    component_id: "__COMPONENT_ID__",
    type_id: "__TYPE_ID__",
    display_name: "__NAME__",
}
"@
  }
  "lyrics" {
    $world = "stellatune:plugin/lyrics-plugin@0.1.0"
    $macroName = "export_lyrics_plugin"
    $libCode = @"
use stellatune_wasm_plugin_sdk::prelude::*;

pub struct Plugin;
pub struct Provider;

impl PluginLifecycle for Plugin {}
impl ConfigStateOps for Provider {}

impl LyricsProvider for Provider {
    fn search(&mut self, _keyword: &str) -> SdkResult<Vec<LyricCandidate>> {
        Ok(Vec::new())
    }

    fn fetch(&mut self, _id: &str) -> SdkResult<String> {
        Ok(String::new())
    }
}

impl LyricsPlugin for Plugin {
    type Provider = Provider;
    const TYPE_ID: &'static str = "__TYPE_ID__";
    const DISPLAY_NAME: &'static str = "__NAME__";

    fn create_provider(&mut self) -> SdkResult<Self::Provider> {
        Ok(Provider)
    }
}

fn create_plugin() -> SdkResult<Plugin> {
    Ok(Plugin)
}

stellatune_wasm_plugin_sdk::__MACRO__! {
    export: generated_export,
    plugin_type: crate::Plugin,
    create: crate::create_plugin,
    plugin_id: "__PLUGIN_ID__",
    component_id: "__COMPONENT_ID__",
    type_id: "__TYPE_ID__",
    display_name: "__NAME__",
}
"@
  }
  "output-sink" {
    $world = "stellatune:plugin/output-sink-plugin@0.1.0"
    $macroName = "export_output_sink_plugin"
    $libCode = @"
use stellatune_wasm_plugin_sdk::prelude::*;

pub struct Plugin;
pub struct Session;

impl PluginLifecycle for Plugin {}
impl ConfigStateOps for Session {}

impl OutputSinkSession for Session {
    fn list_targets_json(&mut self) -> SdkResult<String> {
        Ok("[]".to_string())
    }

    fn negotiate_spec_json(&mut self, _target_json: &str, desired: AudioSpec) -> SdkResult<NegotiatedSpec> {
        Ok(NegotiatedSpec {
            spec: desired,
            preferred_chunk_frames: 1024,
            prefer_track_rate: true,
        })
    }

    fn describe_hot_path(&mut self, _spec: AudioSpec) -> SdkResult<Option<CoreModuleSpec>> {
        Ok(None)
    }

    fn open_json(&mut self, _target_json: &str, _spec: AudioSpec) -> SdkResult<()> {
        Ok(())
    }

    fn write_interleaved_f32(&mut self, channels: u16, interleaved_f32le: &[u8]) -> SdkResult<u32> {
        if channels == 0 {
            return Ok(0);
        }
        let bytes_per_frame = channels as usize * 4;
        Ok((interleaved_f32le.len() / bytes_per_frame) as u32)
    }

    fn query_status(&mut self) -> SdkResult<OutputSinkStatus> {
        Ok(OutputSinkStatus { queued_samples: 0, running: true })
    }

    fn flush(&mut self) -> SdkResult<()> {
        Ok(())
    }

    fn reset(&mut self) -> SdkResult<()> {
        Ok(())
    }
}

impl OutputSinkPlugin for Plugin {
    type Session = Session;
    const TYPE_ID: &'static str = "__TYPE_ID__";
    const DISPLAY_NAME: &'static str = "__NAME__";

    fn create_session(&mut self) -> SdkResult<Self::Session> {
        Ok(Session)
    }
}

fn create_plugin() -> SdkResult<Plugin> {
    Ok(Plugin)
}

stellatune_wasm_plugin_sdk::__MACRO__! {
    export: generated_export,
    plugin_type: crate::Plugin,
    create: crate::create_plugin,
    plugin_id: "__PLUGIN_ID__",
    component_id: "__COMPONENT_ID__",
    type_id: "__TYPE_ID__",
    display_name: "__NAME__",
}
"@
  }
  "dsp" {
    $world = "stellatune:plugin/dsp-plugin@0.1.0"
    $macroName = "export_dsp_plugin"
    $libCode = @"
use stellatune_wasm_plugin_sdk::prelude::*;

pub struct Plugin;
pub struct Processor;

impl PluginLifecycle for Plugin {}
impl ConfigStateOps for Processor {}

impl DspProcessor for Processor {
    fn describe_hot_path(&mut self, _spec: AudioSpec) -> SdkResult<Option<CoreModuleSpec>> {
        Ok(None)
    }

    fn process_interleaved_f32(&mut self, _channels: u16, interleaved_f32le: &[u8]) -> SdkResult<Vec<u8>> {
        Ok(interleaved_f32le.to_vec())
    }

    fn supported_layouts(&self) -> u32 {
        0
    }

    fn output_channels(&self) -> u16 {
        0
    }
}

impl DspPlugin for Plugin {
    type Processor = Processor;
    const TYPE_ID: &'static str = "__TYPE_ID__";
    const DISPLAY_NAME: &'static str = "__NAME__";

    fn create_processor(&mut self, _spec: AudioSpec) -> SdkResult<Self::Processor> {
        Ok(Processor)
    }
}

fn create_plugin() -> SdkResult<Plugin> {
    Ok(Plugin)
}

stellatune_wasm_plugin_sdk::__MACRO__! {
    export: generated_export,
    plugin_type: crate::Plugin,
    create: crate::create_plugin,
    plugin_id: "__PLUGIN_ID__",
    component_id: "__COMPONENT_ID__",
    type_id: "__TYPE_ID__",
    display_name: "__NAME__",
}
"@
  }
}

$cargoToml = @"
[package]
name = "$crateName"
version = "$Version"
edition = "2024"

[lib]
crate-type = ["cdylib"]

[dependencies]
stellatune-wasm-plugin-sdk = { path = "../../crates/stellatune-wasm-plugin-sdk" }

[workspace]
"@

$manifestJson = @"
{
  "schema_version": 1,
  "id": "__PLUGIN_ID__",
  "name": "__NAME__",
  "version": "$Version",
  "api_version": 1,
  "components": [
    {
      "id": "__COMPONENT_ID__",
      "path": "wasm/__CRATE__.wasm",
      "world": "__WORLD__",
      "abilities": [
        {
          "kind": "__ABILITY_KIND__",
          "type_id": "__TYPE_ID__"
        }
      ]
    }
  ]
}
"@

$abilityKind = $Ability.Replace("-", "_")

$cargoToml = $cargoToml.Replace("__NAME__", $Name)
$libCode = $libCode.Replace("__TYPE_ID__", $TypeId).
  Replace("__NAME__", $Name).
  Replace("__PLUGIN_ID__", $PluginId).
  Replace("__COMPONENT_ID__", $ComponentId).
  Replace("__MACRO__", $macroName)
$manifestJson = $manifestJson.Replace("__PLUGIN_ID__", $PluginId).
  Replace("__NAME__", $Name).
  Replace("__COMPONENT_ID__", $ComponentId).
  Replace("__CRATE__", $crateName).
  Replace("__WORLD__", $world).
  Replace("__TYPE_ID__", $TypeId).
  Replace("__ABILITY_KIND__", $abilityKind)

Set-Content -Path (Join-Path $projectDir "Cargo.toml") -Value $cargoToml -NoNewline
Set-Content -Path (Join-Path $projectDir "src/lib.rs") -Value $libCode -NoNewline
Set-Content -Path (Join-Path $projectDir "plugin.json") -Value $manifestJson -NoNewline
$readme = @'
# __NAME__

Generated by tools/wasm-plugin-sdk/new-plugin.ps1.

## Build

```powershell
cargo build --release --target wasm32-wasip2
```

## Package

```powershell
tools/wasm-plugin-sdk/package-plugin.ps1 -ProjectDir __PROJECT_DIR__
```
'@
$readme = $readme.Replace("__NAME__", $Name).Replace("__PROJECT_DIR__", $projectDir)
Set-Content -Path (Join-Path $projectDir "README.md") -Value $readme -NoNewline

Write-Host "Scaffold created at $projectDir"
