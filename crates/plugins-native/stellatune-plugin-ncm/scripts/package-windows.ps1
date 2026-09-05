param([string]$OutDir = "dist")
$ErrorActionPreference = "Stop"
$pluginRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$outputRoot = [System.IO.Path]::GetFullPath((Join-Path $pluginRoot $OutDir))
if (-not $outputRoot.StartsWith($pluginRoot + [System.IO.Path]::DirectorySeparatorChar, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Package output must be inside the plugin project directory."
}
$manifest = Get-Content -LiteralPath (Join-Path $pluginRoot "manifest.json") -Raw | ConvertFrom-Json
$workspace = (Resolve-Path (Join-Path $pluginRoot "../../..")).Path
cargo build --manifest-path (Join-Path $pluginRoot "host/Cargo.toml") --release
if ($LASTEXITCODE -ne 0) { throw "NCM host build failed" }
New-Item -ItemType Directory -Path (Join-Path $pluginRoot "bin") -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $workspace "target/release/stellatune-ncm-host.exe") -Destination (Join-Path $pluginRoot "bin/stellatune-ncm-host.exe")
New-Item -ItemType Directory -Path $outputRoot -Force | Out-Null
$artifact = Join-Path $outputRoot "$($manifest.id)-$($manifest.version).zip"
$payload = @("manifest.json", "plugin.mjs", "bin") | ForEach-Object { Join-Path $pluginRoot $_ }
Compress-Archive -LiteralPath $payload -DestinationPath $artifact -CompressionLevel Optimal -Force
Write-Output $artifact
