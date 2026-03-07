param(
    [ValidateSet("Debug", "Release")]
    [string]$Configuration = "Release",
    [string]$BuildTarget = "wasm32-wasip2",
    [string]$FfmpegExePath = "",
    [string]$FfprobeExePath = "",
    [string]$FfmpegDownloadUrl = "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip",
    [string]$FfmpegCacheDir = "",
    [switch]$SkipFfmpegDownload,
    [string]$OutDir = ""
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

function Resolve-ExecutablePath {
    param(
        [string]$PathValue,
        [Parameter(Mandatory = $true)]
        [string]$Name
    )

    if ([string]::IsNullOrWhiteSpace($PathValue)) {
        return ""
    }
    if (-not (Test-Path $PathValue)) {
        throw "$Name executable not found: $PathValue"
    }
    return (Resolve-Path $PathValue).Path
}

function Find-ExecutableInDirectory {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RootDir,
        [Parameter(Mandatory = $true)]
        [string]$ExecutableName
    )

    if (-not (Test-Path $RootDir)) {
        return ""
    }

    $matches = Get-ChildItem -Path $RootDir -Recurse -File -Filter $ExecutableName | Sort-Object FullName
    if (-not $matches) {
        return ""
    }

    $binMatch = $matches | Where-Object { $_.FullName -match "\\bin\\" } | Select-Object -First 1
    if ($null -ne $binMatch) {
        return $binMatch.FullName
    }
    return ($matches | Select-Object -First 1).FullName
}

function Ensure-DownloadedFfmpegArchive {
    param(
        [Parameter(Mandatory = $true)]
        [string]$DownloadUrl,
        [Parameter(Mandatory = $true)]
        [string]$CacheDir
    )

    New-Item -ItemType Directory -Force -Path $CacheDir | Out-Null
    $zipPath = Join-Path $CacheDir "ffmpeg-windows.zip"
    $extractDir = Join-Path $CacheDir "ffmpeg-windows"
    $ffmpegPath = Find-ExecutableInDirectory -RootDir $extractDir -ExecutableName "ffmpeg.exe"
    $ffprobePath = Find-ExecutableInDirectory -RootDir $extractDir -ExecutableName "ffprobe.exe"
    if (-not [string]::IsNullOrWhiteSpace($ffmpegPath) -and -not [string]::IsNullOrWhiteSpace($ffprobePath)) {
        return @{
            ffmpeg = (Resolve-Path $ffmpegPath).Path
            ffprobe = (Resolve-Path $ffprobePath).Path
        }
    }

    Write-Host "Downloading FFmpeg archive from:"
    Write-Host "  $DownloadUrl"
    Invoke-WebRequest -Uri $DownloadUrl -OutFile $zipPath -UseBasicParsing

    if (Test-Path $extractDir) {
        Remove-Item -Recurse -Force $extractDir
    }
    Expand-Archive -Path $zipPath -DestinationPath $extractDir -Force

    $ffmpegPath = Find-ExecutableInDirectory -RootDir $extractDir -ExecutableName "ffmpeg.exe"
    $ffprobePath = Find-ExecutableInDirectory -RootDir $extractDir -ExecutableName "ffprobe.exe"
    if ([string]::IsNullOrWhiteSpace($ffmpegPath) -or [string]::IsNullOrWhiteSpace($ffprobePath)) {
        throw "Downloaded FFmpeg archive does not contain ffmpeg.exe + ffprobe.exe"
    }

    return @{
        ffmpeg = (Resolve-Path $ffmpegPath).Path
        ffprobe = (Resolve-Path $ffprobePath).Path
    }
}

function Resolve-FfmpegBinaries {
    param(
        [string]$FfmpegExePath,
        [string]$FfprobeExePath,
        [string]$FfmpegDownloadUrl,
        [string]$FfmpegCacheDir,
        [switch]$SkipFfmpegDownload,
        [string]$OutDir
    )

    $resolvedFfmpeg = Resolve-ExecutablePath -PathValue $FfmpegExePath -Name "ffmpeg"
    $resolvedFfprobe = Resolve-ExecutablePath -PathValue $FfprobeExePath -Name "ffprobe"
    if (-not [string]::IsNullOrWhiteSpace($resolvedFfmpeg) -and -not [string]::IsNullOrWhiteSpace($resolvedFfprobe)) {
        return @{
            ffmpeg = $resolvedFfmpeg
            ffprobe = $resolvedFfprobe
        }
    }

    if ($SkipFfmpegDownload) {
        throw "ffmpeg.exe / ffprobe.exe not provided and -SkipFfmpegDownload is set"
    }

    if ([string]::IsNullOrWhiteSpace($FfmpegCacheDir)) {
        $FfmpegCacheDir = Join-Path $OutDir ".ffmpeg-cache"
    }
    $FfmpegCacheDir = (New-Item -ItemType Directory -Force -Path $FfmpegCacheDir).FullName
    return Ensure-DownloadedFfmpegArchive -DownloadUrl $FfmpegDownloadUrl -CacheDir $FfmpegCacheDir
}

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$PluginRoot = (Resolve-Path (Join-Path $ScriptDir "..")).Path
$DecoderManifestPath = Join-Path $PluginRoot "decoder\Cargo.toml"
$EncoderManifestPath = Join-Path $PluginRoot "encoder\Cargo.toml"
$PluginJsonPath = Join-Path $PluginRoot "plugin.json"
$RepoRoot = (Resolve-Path (Join-Path $PluginRoot "..\..\..")).Path
$CargoTargetDir = Join-Path $RepoRoot "target"
$ProfileDir = Get-ProfileDir -Configuration $Configuration

if (-not (Test-Path $DecoderManifestPath)) {
    throw "decoder manifest not found: $DecoderManifestPath"
}
if (-not (Test-Path $EncoderManifestPath)) {
    throw "encoder manifest not found: $EncoderManifestPath"
}
if (-not (Test-Path $PluginJsonPath)) {
    throw "plugin.json not found: $PluginJsonPath"
}

if ([string]::IsNullOrWhiteSpace($OutDir)) {
    $OutDir = Join-Path $CargoTargetDir "plugins"
}
$OutDir = (New-Item -ItemType Directory -Force -Path $OutDir).FullName

$resolvedBins = Resolve-FfmpegBinaries `
    -FfmpegExePath $FfmpegExePath `
    -FfprobeExePath $FfprobeExePath `
    -FfmpegDownloadUrl $FfmpegDownloadUrl `
    -FfmpegCacheDir $FfmpegCacheDir `
    -SkipFfmpegDownload:$SkipFfmpegDownload `
    -OutDir $OutDir
$resolvedFfmpegExePath = [string]$resolvedBins.ffmpeg
$resolvedFfprobeExePath = [string]$resolvedBins.ffprobe

Write-Host "Using ffmpeg binary:"
Write-Host "  $resolvedFfmpegExePath"
Write-Host "Using ffprobe binary:"
Write-Host "  $resolvedFfprobeExePath"

$pluginManifest = Get-Content $PluginJsonPath -Raw | ConvertFrom-Json
$pluginId = [string]$pluginManifest.id
$pluginVersion = [string]$pluginManifest.version
if ([string]::IsNullOrWhiteSpace($pluginId) -or [string]::IsNullOrWhiteSpace($pluginVersion)) {
    throw "plugin.json missing id or version"
}

$prevCargoTargetDir = $env:CARGO_TARGET_DIR
$env:CARGO_TARGET_DIR = $CargoTargetDir

try {
    Push-Location $RepoRoot

    $manifestsToBuild = @($DecoderManifestPath, $EncoderManifestPath)
    foreach ($manifestPath in $manifestsToBuild) {
        $buildArgs = @("build", "--manifest-path", $manifestPath, "--target", $BuildTarget)
        if ($Configuration -eq "Release") {
            $buildArgs += "--release"
        }
        Invoke-Cargo -CommandArgs $buildArgs
    }

    $wasmBuildDir = Join-Path (Join-Path $CargoTargetDir $BuildTarget) $ProfileDir
    $stageDir = Join-Path $OutDir "stellatune-plugin-ffmpeg-stage"
    if (Test-Path $stageDir) {
        Remove-Item -Recurse -Force $stageDir
    }
    New-Item -ItemType Directory -Force -Path $stageDir | Out-Null
    New-Item -ItemType Directory -Force -Path (Join-Path $stageDir "bin") | Out-Null
    Copy-Item -Path $PluginJsonPath -Destination (Join-Path $stageDir "plugin.json") -Force

    foreach ($component in $pluginManifest.components) {
        $relativePath = [string]$component.path
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

    Copy-Item -Path $resolvedFfmpegExePath -Destination (Join-Path $stageDir "bin\ffmpeg.exe") -Force
    Copy-Item -Path $resolvedFfprobeExePath -Destination (Join-Path $stageDir "bin\ffprobe.exe") -Force

    $zipStem = Get-SafeFileName("$pluginId-$pluginVersion-$BuildTarget-native-$ProfileDir")
    $zipPath = Join-Path $OutDir "$zipStem.zip"
    if (Test-Path $zipPath) {
        Remove-Item -Force $zipPath
    }
    Compress-Archive -Path (Join-Path $stageDir "*") -DestinationPath $zipPath -CompressionLevel Optimal

    Write-Host ""
    Write-Host "Package ready:"
    Write-Host "  $zipPath"
    Write-Host ""
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
