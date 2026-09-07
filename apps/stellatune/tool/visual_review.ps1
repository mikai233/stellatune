param([switch]$Native, [ValidateSet('home', 'library', 'library-albums', 'settings')][string]$Page = 'home', [ValidateSet('dusk', 'mist', 'graphite', 'nebula', 'sunroom', 'daylight', 'lavender', 'celadon')][string]$Theme = 'daylight')
$ErrorActionPreference = 'Stop'
$appRoot = Split-Path -Parent $PSScriptRoot
Push-Location $appRoot
try {
    flutter test test/home_visual_test.dart --reporter expanded
    if ($LASTEXITCODE -ne 0) { throw 'Visual widget tests failed.' }
    if ($Native) {
        flutter build windows --debug -t tool/visual_preview.dart --dart-define=VISUAL_CAPTURE=true "--dart-define=VISUAL_PAGE=$Page" "--dart-define=VISUAL_THEME=$Theme"
        if ($LASTEXITCODE -ne 0) { throw 'Native preview build failed.' }
        $previewExe = Join-Path $appRoot 'build/windows/x64/runner/Debug/stellatune.exe'
        $previewProcess = Start-Process -FilePath $previewExe -WorkingDirectory $appRoot -WindowStyle Hidden -PassThru -Wait
        if ($previewProcess.ExitCode -ne 0) { throw 'Native visual capture failed.' }
    }
    Write-Output "Review PNGs in $(Join-Path $appRoot 'build/visual-review')"
} finally {
    Pop-Location
}
