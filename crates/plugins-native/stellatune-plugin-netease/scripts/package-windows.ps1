param(
    [ValidateSet("Debug", "Release")]
    [string]$Configuration = "Release",
    [string]$BuildTarget = "wasm32-wasip2",
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
        [string[]]$CommandArgs
    )

    & cargo @CommandArgs
    if ($LASTEXITCODE -ne 0) {
        throw "cargo command failed: cargo $($CommandArgs -join ' ')"
    }
}

function Invoke-Npm {
    param(
        [Parameter(Mandatory = $true)]
        [string[]]$CommandArgs
    )

    & npm @CommandArgs
    if ($LASTEXITCODE -ne 0) {
        throw "npm command failed: npm $($CommandArgs -join ' ')"
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
$SourceManifestPath = Join-Path $PluginCrateDir "source\Cargo.toml"
$DecoderManifestPath = Join-Path $PluginCrateDir "decoder\Cargo.toml"
$PluginJsonPath = Join-Path $PluginCrateDir "plugin.json"
$RepoRoot = (Resolve-Path (Join-Path $PluginCrateDir "..\..\..")).Path
$SidecarRoot = Join-Path $RepoRoot "tools\stellatune-ncm-sidecar"
$CargoTargetDir = Join-Path $RepoRoot "target"
$ProfileDir = Get-ProfileDir -Configuration $Configuration

if (-not (Test-Path $SourceManifestPath)) {
    throw "source manifest not found: $SourceManifestPath"
}
if (-not (Test-Path $DecoderManifestPath)) {
    throw "decoder manifest not found: $DecoderManifestPath"
}
if (-not (Test-Path $PluginJsonPath)) {
    throw "plugin.json not found: $PluginJsonPath"
}
if (-not (Test-Path $SidecarRoot)) {
    throw "sidecar directory not found: $SidecarRoot"
}

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

    if (-not $SkipBuild) {
        $manifestsToBuild = @($SourceManifestPath, $DecoderManifestPath)
        foreach ($manifestPath in $manifestsToBuild) {
            $buildArgs = @("build", "--manifest-path", $manifestPath, "--target", $BuildTarget)
            if ($Configuration -eq "Release") {
                $buildArgs += "--release"
            }
            Invoke-Cargo -CommandArgs $buildArgs
        }

        Push-Location $SidecarRoot
        try {
            Invoke-Npm -CommandArgs @("ci")
            Invoke-Npm -CommandArgs @("run", "build:exe")
        }
        finally {
            Pop-Location
        }
    }

    $sidecarExe = Join-Path $SidecarRoot "dist\stellatune-ncm-sidecar.exe"
    if (-not (Test-Path $sidecarExe)) {
        throw "sidecar executable not found: $sidecarExe"
    }

    $wasmBuildDir = Join-Path (Join-Path $CargoTargetDir $BuildTarget) $ProfileDir
    $stageDir = Join-Path $OutDir "stellatune-plugin-netease-stage"
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

    Copy-Item -Path $sidecarExe -Destination (Join-Path $stageDir "bin\stellatune-ncm-sidecar.exe") -Force
    Copy-Item -Path (Join-Path $SidecarRoot "README.md") -Destination (Join-Path $stageDir "bin\stellatune-ncm-sidecar-README.md") -Force

    $zipStem = Get-SafeFileName("$($pluginManifest.id)-$($pluginManifest.version)-$BuildTarget-native-$($ProfileDir)")
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
