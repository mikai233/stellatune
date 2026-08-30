param(
    [string]$OutDir = "dist"
)

$ErrorActionPreference = "Stop"
$pluginRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$outputRoot = [System.IO.Path]::GetFullPath((Join-Path $pluginRoot $OutDir))
$stage = Join-Path $outputRoot "stellatune-plugin-netease-v2"
$artifact = Join-Path $outputRoot "dev.stellatune.source.netease-0.2.0.zip"

New-Item -ItemType Directory -Path $outputRoot -Force | Out-Null
if (Test-Path -LiteralPath $stage) {
    Remove-Item -LiteralPath $stage -Recurse -Force
}
New-Item -ItemType Directory -Path (Join-Path $stage "ui") -Force | Out-Null

Copy-Item -LiteralPath (Join-Path $pluginRoot "manifest.json") -Destination $stage
Copy-Item -LiteralPath (Join-Path $pluginRoot "plugin.mjs") -Destination $stage
Copy-Item -LiteralPath (Join-Path $pluginRoot "source-config.schema.json") -Destination $stage
Copy-Item -Path (Join-Path $pluginRoot "ui\*") -Destination (Join-Path $stage "ui") -Recurse -Force

if (Test-Path -LiteralPath $artifact) {
    Remove-Item -LiteralPath $artifact -Force
}
Compress-Archive -Path (Join-Path $stage "*") -DestinationPath $artifact -CompressionLevel Optimal
Write-Output $artifact
