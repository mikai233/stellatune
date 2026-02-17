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
        [string[]]$CmdArgs
    )

    & cargo @CmdArgs
    if ($LASTEXITCODE -ne 0) {
        throw "cargo command failed: cargo $($CmdArgs -join ' ')"
    }
}

function Invoke-Npm {
    param(
        [Parameter(Mandatory = $true)]
        [string[]]$CmdArgs
    )

    & npm @CmdArgs
    if ($LASTEXITCODE -ne 0) {
        throw "npm command failed: npm $($CmdArgs -join ' ')"
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
$PluginManifestPath = Join-Path $ScriptDir "..\Cargo.toml"

if (-not (Test-Path $PluginManifestPath)) {
    throw "plugin manifest not found: $PluginManifestPath"
}

$PluginManifestPath = (Resolve-Path $PluginManifestPath).Path
$PluginCrateDir = Split-Path -Parent $PluginManifestPath
$RepoRoot = (Resolve-Path (Join-Path $PluginCrateDir "..\..\..")).Path
$ProfileDir = Get-ProfileDir -Configuration $Configuration
$CargoTargetDir = Join-Path $RepoRoot "target"
$SidecarRoot = Join-Path $RepoRoot "tools\stellatune-ncm-sidecar"

if (-not (Test-Path $SidecarRoot)) {
    throw "sidecar directory not found: $SidecarRoot"
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
        Invoke-Cargo -CmdArgs (@("build", "--manifest-path", $PluginManifestPath) + $commonArgs)
    }

    Push-Location $SidecarRoot
    try {
        Invoke-Npm -CmdArgs @("ci")
        Invoke-Npm -CmdArgs @("run", "build:exe")
    }
    finally {
        Pop-Location
    }

    $sidecarExe = Join-Path $SidecarRoot "dist\stellatune-ncm-sidecar.exe"
    if (-not (Test-Path $sidecarExe)) {
        throw "sidecar executable not found: $sidecarExe"
    }

    $targetRoot = $CargoTargetDir
    if (-not [string]::IsNullOrWhiteSpace($Target)) {
        $targetRoot = Join-Path $targetRoot $Target
    }
    $buildDir = Join-Path $targetRoot $ProfileDir

    $pluginDll = Join-Path $buildDir "stellatune_plugin_netease.dll"
    if (-not (Test-Path $pluginDll)) {
        throw "plugin dll not found: $pluginDll"
    }
    $pluginPdb = Join-Path $buildDir "stellatune_plugin_netease.pdb"

    $stageDir = Join-Path $OutDir "stellatune-plugin-netease-stage"
    if (Test-Path $stageDir) {
        Remove-Item -Recurse -Force $stageDir
    }
    New-Item -ItemType Directory -Force -Path $stageDir | Out-Null
    New-Item -ItemType Directory -Force -Path (Join-Path $stageDir "bin") | Out-Null

    Copy-Item -Path $pluginDll -Destination (Join-Path $stageDir "stellatune_plugin_netease.dll") -Force
    if (Test-Path $pluginPdb) {
        Copy-Item -Path $pluginPdb -Destination (Join-Path $stageDir "stellatune_plugin_netease.pdb") -Force
    }

    Copy-Item -Path $sidecarExe -Destination (Join-Path $stageDir "bin\stellatune-ncm-sidecar.exe") -Force
    Copy-Item -Path (Join-Path $SidecarRoot "README.md") -Destination (Join-Path $stageDir "bin\stellatune-ncm-sidecar-README.md") -Force

    $targetLabel = if ([string]::IsNullOrWhiteSpace($Target)) { "native" } else { $Target }
    $configLabel = $Configuration.ToLowerInvariant()
    $zipName = "stellatune-plugin-netease-$targetLabel-$configLabel.zip"
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
