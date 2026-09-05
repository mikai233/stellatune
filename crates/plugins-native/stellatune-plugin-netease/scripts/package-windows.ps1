param(
    [string]$OutDir = "dist"
)

$ErrorActionPreference = "Stop"
$pluginRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$outputRoot = [System.IO.Path]::GetFullPath((Join-Path $pluginRoot $OutDir))
if (-not $outputRoot.StartsWith($pluginRoot.Path + [System.IO.Path]::DirectorySeparatorChar, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Package output must be inside the plugin project directory."
}
$stage = Join-Path $outputRoot "stellatune-plugin-netease-v2"
$manifest = Get-Content -LiteralPath (Join-Path $pluginRoot "manifest.json") -Raw | ConvertFrom-Json
$artifact = Join-Path $outputRoot "$($manifest.id)-$($manifest.version).zip"

& npm.cmd --prefix (Join-Path $pluginRoot "ui-web") run build
if ($LASTEXITCODE -ne 0) { throw "Plugin build failed." }

New-Item -ItemType Directory -Path $outputRoot -Force | Out-Null
if (Test-Path -LiteralPath $stage) {
    $resolvedStage = (Resolve-Path -LiteralPath $stage).Path
    if (-not $resolvedStage.StartsWith($outputRoot + [System.IO.Path]::DirectorySeparatorChar, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Package stage is outside the output directory."
    }
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
