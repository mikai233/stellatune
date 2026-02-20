param(
  [Parameter(Mandatory = $true)]
  [string]$ProjectDir,
  [string]$BuildTarget = "wasm32-wasip2",
  [string]$Profile = "release",
  [string]$OutDir = "target/plugins"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$projectDir = (Resolve-Path $ProjectDir).Path
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "../..")).Path
$outRoot = if ([System.IO.Path]::IsPathRooted($OutDir)) {
  $OutDir
} else {
  Join-Path $repoRoot $OutDir
}
$manifestPath = Join-Path $projectDir "plugin.json"
if (-not (Test-Path $manifestPath)) {
  throw "missing plugin manifest: $manifestPath"
}

$manifest = Get-Content $manifestPath -Raw | ConvertFrom-Json
if (-not $manifest.id) {
  throw "manifest.id is required"
}

$crateToml = Join-Path $projectDir "Cargo.toml"
if (-not (Test-Path $crateToml)) {
  throw "missing Cargo.toml: $crateToml"
}

$crateName = (Get-Content $crateToml | Where-Object { $_ -match '^name\s*=' } | Select-Object -First 1)
if (-not $crateName) {
  throw "failed to parse crate name from $crateToml"
}
$crateName = ($crateName -replace '^name\s*=\s*"', "") -replace '"\s*$', ""

$pluginRoot = Join-Path $outRoot $manifest.id
if (Test-Path $pluginRoot) {
  Remove-Item -Recurse -Force $pluginRoot
}
New-Item -ItemType Directory -Path $pluginRoot | Out-Null

Copy-Item $manifestPath (Join-Path $pluginRoot "plugin.json")

foreach ($component in $manifest.components) {
  $relativePath = [string]$component.path
  if ([string]::IsNullOrWhiteSpace($relativePath)) {
    throw "component.path is empty in manifest"
  }

  $sourcePath = Join-Path $projectDir $relativePath
  if (-not (Test-Path $sourcePath)) {
    $fallbackFileNames = @(
      "$crateName.wasm",
      (($crateName -replace "-", "_") + ".wasm")
    )
    $fallbackTargetRoots = @(
      (Join-Path $projectDir "target"),
      (Join-Path $repoRoot "target"),
      (Join-Path (Get-Location).Path "target")
    ) | Select-Object -Unique

    $fallbackPath = $null
    foreach ($targetRoot in $fallbackTargetRoots) {
      foreach ($fileName in $fallbackFileNames) {
        $candidate = Join-Path $targetRoot "$BuildTarget/$Profile/$fileName"
        if (Test-Path $candidate) {
          $fallbackPath = $candidate
          break
        }
      }
      if ($fallbackPath) { break }
    }
    if (Test-Path $fallbackPath) {
      $sourcePath = $fallbackPath
    } else {
      throw "component wasm not found: $sourcePath"
    }
  }

  $destPath = Join-Path $pluginRoot $relativePath
  $destDir = Split-Path -Parent $destPath
  if (-not (Test-Path $destDir)) {
    New-Item -ItemType Directory -Path $destDir | Out-Null
  }
  Copy-Item $sourcePath $destPath -Force
}

function Get-SafeFileName([string]$name) {
  $invalid = [System.IO.Path]::GetInvalidFileNameChars()
  $safe = $name
  foreach ($ch in $invalid) {
    $safe = $safe.Replace($ch, '_')
  }
  return $safe
}

$zipBaseName = Get-SafeFileName("$($manifest.id)-$($manifest.version)")
$zipPath = Join-Path $outRoot ($zipBaseName + ".zip")
if (Test-Path $zipPath) {
  Remove-Item -Force $zipPath
}
Compress-Archive -Path (Join-Path $pluginRoot "*") -DestinationPath $zipPath -CompressionLevel Optimal -Force

Write-Host "Packaged plugin root: $pluginRoot"
Write-Host "Packaged plugin zip:  $zipPath"
