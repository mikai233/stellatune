param(
    [ValidateSet("Debug", "Release")]
    [string]$Configuration = "Release",
    [string]$Target = "",
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

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Resolve-Path (Join-Path $ScriptDir "..\..\..")
$ProfileDir = Get-ProfileDir -Configuration $Configuration
$CargoTargetDir = Join-Path $RepoRoot "target"

$PluginManifestPath = Join-Path $RepoRoot "crates\plugins-native\stellatune-plugin-asio\Cargo.toml"
$AsioHostManifestPath = Join-Path $RepoRoot "crates\plugins-native\stellatune-asio-host\Cargo.toml"

if (-not (Test-Path $PluginManifestPath)) {
    throw "plugin manifest not found: $PluginManifestPath"
}
if (-not (Test-Path $AsioHostManifestPath)) {
    throw "ASIO host manifest not found: $AsioHostManifestPath"
}

if ([string]::IsNullOrWhiteSpace($OutDir)) {
    $OutDir = Join-Path $CargoTargetDir "plugins"
}
$OutDir = (New-Item -ItemType Directory -Force -Path $OutDir).FullName

$prevCargoTargetDir = $env:CARGO_TARGET_DIR
$env:CARGO_TARGET_DIR = $CargoTargetDir

try {
    Push-Location $RepoRoot

    $commonArgs = @()
    if (-not [string]::IsNullOrWhiteSpace($Target)) {
        $commonArgs += @("--target", $Target)
    }
    if ($Configuration -eq "Release") {
        $commonArgs += "--release"
    }

    if (-not $SkipBuild) {
        Invoke-Cargo -Args (@("build", "--manifest-path", $PluginManifestPath) + $commonArgs)
        Invoke-Cargo -Args (@(
                "build",
                "--manifest-path", $AsioHostManifestPath,
                "--features", "asio"
            ) + $commonArgs)
    }

    $targetRoot = $CargoTargetDir
    if (-not [string]::IsNullOrWhiteSpace($Target)) {
        $targetRoot = Join-Path $targetRoot $Target
    }
    $buildDir = Join-Path $targetRoot $ProfileDir

    $pluginDll = Join-Path $buildDir "stellatune_plugin_asio.dll"
    if (-not (Test-Path $pluginDll)) {
        throw "plugin dll not found: $pluginDll"
    }

    $pluginPdb = Join-Path $buildDir "stellatune_plugin_asio.pdb"
    $sidecarExe = Join-Path $buildDir "stellatune-asio-host.exe"
    if (-not (Test-Path $sidecarExe)) {
        throw "ASIO sidecar not found: $sidecarExe"
    }

    $stageDir = Join-Path $OutDir "stellatune-plugin-asio-stage"
    if (Test-Path $stageDir) {
        Remove-Item -Recurse -Force $stageDir
    }
    New-Item -ItemType Directory -Force -Path $stageDir | Out-Null
    New-Item -ItemType Directory -Force -Path (Join-Path $stageDir "bin") | Out-Null

    Copy-Item -Path $pluginDll -Destination (Join-Path $stageDir "stellatune_plugin_asio.dll") -Force
    if (Test-Path $pluginPdb) {
        Copy-Item -Path $pluginPdb -Destination (Join-Path $stageDir "stellatune_plugin_asio.pdb") -Force
    }
    Copy-Item -Path $sidecarExe -Destination (Join-Path $stageDir "bin\stellatune-asio-host.exe") -Force

    $targetLabel = if ([string]::IsNullOrWhiteSpace($Target)) { "native" } else { $Target }
    $configLabel = $Configuration.ToLowerInvariant()
    $zipName = "stellatune-plugin-asio-$targetLabel-$configLabel.zip"
    $zipPath = Join-Path $OutDir $zipName
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
