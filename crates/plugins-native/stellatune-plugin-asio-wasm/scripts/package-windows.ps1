param(
    [ValidateSet("Debug", "Release")]
    [string]$Configuration = "Release",
    [string]$BuildTarget = "wasm32-wasip2",
    [string]$HostTarget = "",
    [string]$OutDir = "",
    [switch]$SkipBuild
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if ($env:OS -ne "Windows_NT") {
    throw "This script only supports Windows packaging."
}

function Invoke-Cargo {
    param(
        [Parameter(Mandatory = $true)]
        [string[]]$Args
    )

    & cargo @Args
    if ($LASTEXITCODE -ne 0) {
        throw "cargo command failed: cargo $($Args -join ' ')"
    }
}

function Get-ProfileDir {
    param([string]$Configuration)
    if ($Configuration -eq "Release") {
        return "release"
    }
    return "debug"
}

function Get-SafeFileName {
    param([Parameter(Mandatory = $true)][string]$Name)
    $invalidChars = [System.IO.Path]::GetInvalidFileNameChars()
    $safe = $Name
    foreach ($ch in $invalidChars) {
        $safe = $safe.Replace($ch, "_")
    }
    return $safe
}

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$PluginCrateDir = (Resolve-Path (Join-Path $ScriptDir "..")).Path
$PluginManifestPath = Join-Path $PluginCrateDir "Cargo.toml"
$PluginJsonPath = Join-Path $PluginCrateDir "plugin.json"
$AsioHostManifestPath = Join-Path $PluginCrateDir "..\stellatune-asio-host\Cargo.toml"

if (-not (Test-Path $PluginManifestPath)) {
    throw "plugin manifest not found: $PluginManifestPath"
}
if (-not (Test-Path $PluginJsonPath)) {
    throw "plugin.json not found: $PluginJsonPath"
}
if (-not (Test-Path $AsioHostManifestPath)) {
    throw "ASIO host manifest not found: $AsioHostManifestPath"
}
$AsioHostManifestPath = (Resolve-Path $AsioHostManifestPath).Path

$RepoRoot = (Resolve-Path (Join-Path $PluginCrateDir "..\..\..")).Path
$CargoTargetDir = Join-Path $RepoRoot "target"
$ProfileDir = Get-ProfileDir -Configuration $Configuration

if ([string]::IsNullOrWhiteSpace($OutDir)) {
    $OutDir = Join-Path $CargoTargetDir "plugins"
}
$OutDir = (New-Item -ItemType Directory -Force -Path $OutDir).FullName

$pluginManifest = Get-Content $PluginJsonPath -Raw | ConvertFrom-Json
if (-not $pluginManifest.id) {
    throw "plugin.json missing id"
}
if (-not $pluginManifest.version) {
    throw "plugin.json missing version"
}
if (-not $pluginManifest.components -or $pluginManifest.components.Count -eq 0) {
    throw "plugin.json has no components"
}

$prevCargoTargetDir = $env:CARGO_TARGET_DIR
$env:CARGO_TARGET_DIR = $CargoTargetDir

try {
    Push-Location $RepoRoot

    $wasmBuildArgs = @("build", "--manifest-path", $PluginManifestPath, "--target", $BuildTarget)
    if ($Configuration -eq "Release") {
        $wasmBuildArgs += "--release"
    }

    $hostBuildArgs = @("build", "--manifest-path", $AsioHostManifestPath, "--features", "asio")
    if (-not [string]::IsNullOrWhiteSpace($HostTarget)) {
        $hostBuildArgs += @("--target", $HostTarget)
    }
    if ($Configuration -eq "Release") {
        $hostBuildArgs += "--release"
    }

    if (-not $SkipBuild) {
        Invoke-Cargo -Args $wasmBuildArgs
        Invoke-Cargo -Args $hostBuildArgs
    }

    $wasmBuildDir = Join-Path (Join-Path $CargoTargetDir $BuildTarget) $ProfileDir
    $hostTargetRoot = $CargoTargetDir
    if (-not [string]::IsNullOrWhiteSpace($HostTarget)) {
        $hostTargetRoot = Join-Path $hostTargetRoot $HostTarget
    }
    $hostBuildDir = Join-Path $hostTargetRoot $ProfileDir

    $sidecarExe = Join-Path $hostBuildDir "stellatune-asio-host.exe"
    if (-not (Test-Path $sidecarExe)) {
        throw "ASIO sidecar not found: $sidecarExe"
    }
    $sidecarPdb = Join-Path $hostBuildDir "stellatune-asio-host.pdb"

    $stageDir = Join-Path $OutDir "stellatune-plugin-asio-wasm-stage"
    if (Test-Path $stageDir) {
        Remove-Item -Recurse -Force $stageDir
    }
    New-Item -ItemType Directory -Force -Path $stageDir | Out-Null
    New-Item -ItemType Directory -Force -Path (Join-Path $stageDir "bin") | Out-Null
    Copy-Item -Path $PluginJsonPath -Destination (Join-Path $stageDir "plugin.json") -Force

    foreach ($component in $pluginManifest.components) {
        $relativePath = [string]$component.path
        if ([string]::IsNullOrWhiteSpace($relativePath)) {
            throw "component.path is empty in plugin.json"
        }

        $fileName = Split-Path -Leaf $relativePath
        $sourcePath = Join-Path $wasmBuildDir $fileName
        if (-not (Test-Path $sourcePath)) {
            throw "component wasm not found: $sourcePath"
        }

        $destinationPath = Join-Path $stageDir $relativePath
        $destinationDir = Split-Path -Parent $destinationPath
        if (-not (Test-Path $destinationDir)) {
            New-Item -ItemType Directory -Force -Path $destinationDir | Out-Null
        }
        Copy-Item -Path $sourcePath -Destination $destinationPath -Force
    }

    Copy-Item -Path $sidecarExe -Destination (Join-Path $stageDir "bin\stellatune-asio-host.exe") -Force
    if (Test-Path $sidecarPdb) {
        Copy-Item -Path $sidecarPdb -Destination (Join-Path $stageDir "bin\stellatune-asio-host.pdb") -Force
    }

    $hostTargetLabel = if ([string]::IsNullOrWhiteSpace($HostTarget)) { "native" } else { $HostTarget }
    $zipStem = Get-SafeFileName("$($pluginManifest.id)-$($pluginManifest.version)-$BuildTarget-$hostTargetLabel-$($ProfileDir)")
    $zipPath = Join-Path $OutDir "$zipStem.zip"
    if (Test-Path $zipPath) {
        Remove-Item -Force $zipPath
    }
    Compress-Archive -Path (Join-Path $stageDir "*") -DestinationPath $zipPath -CompressionLevel Optimal

    Write-Host ""
    Write-Host "Package ready:"
    Write-Host "  $zipPath"
    Write-Host ""
    Write-Host "Install this zip from StellaTune Settings -> Plugins -> Install."
}
finally {
    Pop-Location

    if ($null -eq $prevCargoTargetDir) {
        Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue
    }
    else {
        $env:CARGO_TARGET_DIR = $prevCargoTargetDir
    }
}
