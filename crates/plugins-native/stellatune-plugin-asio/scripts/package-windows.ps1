param([string]$OutDir = "dist", [string]$AsioSdkDir = $env:CPAL_ASIO_DIR)
$ErrorActionPreference = "Stop"
$pluginRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$workspace = (Resolve-Path (Join-Path $pluginRoot "../../..")).Path
$outputRoot = [IO.Path]::GetFullPath((Join-Path $pluginRoot $OutDir))
if (-not $outputRoot.StartsWith($pluginRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
    throw "Package output must be inside the plugin project directory."
}
if ($AsioSdkDir) { $env:CPAL_ASIO_DIR = (Resolve-Path -LiteralPath $AsioSdkDir).Path }
$hostRoot = (Resolve-Path (Join-Path $pluginRoot "../stellatune-asio-host")).Path
$buildRoot = Join-Path $workspace "target/asio-host"
cargo build --manifest-path (Join-Path $hostRoot "Cargo.toml") --locked --release --features asio --target-dir $buildRoot
if ($LASTEXITCODE -ne 0) { throw "ASIO host build failed" }
New-Item -ItemType Directory -Path (Join-Path $pluginRoot "bin") -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $buildRoot "release/stellatune-asio-host.exe") -Destination (Join-Path $pluginRoot "bin/stellatune-asio-host.exe")
if ($AsioSdkDir) {
    New-Item -ItemType Directory -Path (Join-Path $pluginRoot "bin/licenses") -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $AsioSdkDir "LICENSE.txt") -Destination (Join-Path $pluginRoot "bin/licenses/ASIO-SDK.txt")
}
New-Item -ItemType Directory -Path $outputRoot -Force | Out-Null
$manifest = Get-Content -LiteralPath (Join-Path $pluginRoot "manifest.json") -Raw | ConvertFrom-Json
$artifact = Join-Path $outputRoot "$($manifest.id)-$($manifest.version).zip"
$payload = @("manifest.json", "plugin.mjs", "schema", "bin", "README.md", "LICENSE.txt") | ForEach-Object { Join-Path $pluginRoot $_ }
Compress-Archive -LiteralPath $payload -DestinationPath $artifact -CompressionLevel Optimal -Force
Write-Output $artifact
