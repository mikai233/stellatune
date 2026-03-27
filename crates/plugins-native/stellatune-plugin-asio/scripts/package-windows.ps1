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

function Test-AsioSdkRoot {
    param([Parameter(Mandatory = $true)][string]$Path)

    if (-not (Test-Path $Path)) {
        return $false
    }

    $required = @(
        (Join-Path $Path "host\asiodrivers.h"),
        (Join-Path $Path "common\asio.h"),
        (Join-Path $Path "common\asiosys.h")
    )

    foreach ($item in $required) {
        if (-not (Test-Path $item)) {
            return $false
        }
    }

    return $true
}

function Prepare-AsioSdk {
    $explicitSdkDir = $null
    if (-not [string]::IsNullOrWhiteSpace($env:CPAL_ASIO_DIR)) {
        $explicitSdkDir = $env:CPAL_ASIO_DIR
    }
    elseif (-not [string]::IsNullOrWhiteSpace($env:ASIO_SDK_DIR)) {
        $explicitSdkDir = $env:ASIO_SDK_DIR
    }

    if (-not [string]::IsNullOrWhiteSpace($explicitSdkDir)) {
        if (-not (Test-Path $explicitSdkDir)) {
            throw "Configured ASIO SDK path does not exist: $explicitSdkDir"
        }
        $resolved = (Resolve-Path $explicitSdkDir).Path
        if (-not (Test-AsioSdkRoot -Path $resolved)) {
            throw @(
                "Configured ASIO SDK is incomplete: $resolved",
                "Expected files:",
                "  host\\asiodrivers.h",
                "  common\\asio.h",
                "  common\\asiosys.h"
            ) -join [Environment]::NewLine
        }
        $env:CPAL_ASIO_DIR = $resolved
        Write-Host "Using ASIO SDK from environment: $resolved"
        return
    }

    $tempSdkDir = Join-Path ([System.IO.Path]::GetTempPath()) "asio_sdk"
    if (Test-Path $tempSdkDir) {
        $resolvedTemp = (Resolve-Path $tempSdkDir).Path
        if (Test-AsioSdkRoot -Path $resolvedTemp) {
            Write-Host "Using cached ASIO SDK: $resolvedTemp"
            return
        }

        Write-Warning "Detected incomplete cached ASIO SDK at $resolvedTemp"
        Write-Host "Removing stale cache so asio-sys can download a fresh SDK during build..."
        Remove-Item -Recurse -Force $resolvedTemp
    }
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
$prevCpalAsioDir = $env:CPAL_ASIO_DIR

try {
    Push-Location $RepoRoot

    Prepare-AsioSdk

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

    $stageDir = Join-Path $OutDir "stellatune-plugin-asio-stage"
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

    if ($null -eq $prevCpalAsioDir) {
        Remove-Item Env:CPAL_ASIO_DIR -ErrorAction SilentlyContinue
    }
    else {
        $env:CPAL_ASIO_DIR = $prevCpalAsioDir
    }
}
