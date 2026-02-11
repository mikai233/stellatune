param(
    [int]$Iterations = 50,
    [int]$RestartIntervalMs = 4000,
    [int]$BootWaitSec = 30,
    [string]$FlutterDevice = "windows",
    [string]$FlutterBin = "",
    [string]$RustLog = "debug",
    [string]$FlutterProjectDir = "",
    [string]$OutputDir = "",
    [string]$RingDir = "",
    [switch]$KeepFlutterRunning
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if ($env:OS -ne "Windows_NT") {
    throw "This script only supports Windows."
}

if ($Iterations -lt 1) {
    throw "Iterations must be >= 1."
}

if ($RestartIntervalMs -lt 500) {
    throw "RestartIntervalMs should be >= 500."
}

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Resolve-Path (Join-Path $ScriptDir "..\..")

if ([string]::IsNullOrWhiteSpace($FlutterProjectDir)) {
    $FlutterProjectDir = Join-Path $RepoRoot "apps\stellatune"
}
$FlutterProjectDir = (Resolve-Path $FlutterProjectDir).Path

if (-not (Test-Path (Join-Path $FlutterProjectDir "pubspec.yaml"))) {
    throw "Flutter project not found (missing pubspec.yaml): $FlutterProjectDir"
}

if ([string]::IsNullOrWhiteSpace($OutputDir)) {
    $stamp = Get-Date -Format "yyyyMMdd-HHmmss"
    $OutputDir = Join-Path $RepoRoot "target\hot-restart-stress\$stamp"
}
$OutputDir = (New-Item -ItemType Directory -Force -Path $OutputDir).FullName

if ([string]::IsNullOrWhiteSpace($RingDir)) {
    $RingDir = Join-Path $env:TEMP "stellatune\.asio"
}

$logPath = Join-Path $OutputDir "flutter-run.log"
$samplesPath = Join-Path $OutputDir "samples.csv"
$summaryJsonPath = Join-Path $OutputDir "summary.json"
$summaryMdPath = Join-Path $OutputDir "summary.md"
$runtimeTracingLogPath = Join-Path $env:TEMP "stellatune\tracing.log"
if (-not (Test-Path (Split-Path -Parent $runtimeTracingLogPath))) {
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $runtimeTracingLogPath) | Out-Null
}
Set-Content -Path $runtimeTracingLogPath -Value "" -Encoding UTF8

function Resolve-FlutterBin {
    param([string]$Preferred)

    if (-not [string]::IsNullOrWhiteSpace($Preferred)) {
        if (-not (Test-Path $Preferred)) {
            throw "Flutter binary not found: $Preferred"
        }
        return (Resolve-Path $Preferred).Path
    }

    $candidates = New-Object System.Collections.Generic.List[string]

    $flutterBat = Get-Command flutter.bat -ErrorAction SilentlyContinue
    if ($null -ne $flutterBat -and -not [string]::IsNullOrWhiteSpace($flutterBat.Source)) {
        $candidates.Add($flutterBat.Source)
    }

    $flutterCmd = Get-Command flutter -ErrorAction SilentlyContinue
    if ($null -ne $flutterCmd -and -not [string]::IsNullOrWhiteSpace($flutterCmd.Source)) {
        $candidates.Add($flutterCmd.Source)
    }

    if (-not [string]::IsNullOrWhiteSpace($env:USERPROFILE)) {
        $fvmDefaultFlutter = Join-Path $env:USERPROFILE "fvm\default\bin\flutter.bat"
        if (Test-Path $fvmDefaultFlutter) {
            $candidates.Add($fvmDefaultFlutter)
        }
    }

    foreach ($candidate in $candidates) {
        if (Test-Path $candidate) {
            return (Resolve-Path $candidate).Path
        }
    }

    throw "Cannot resolve flutter binary. Set -FlutterBin explicitly (e.g. C:\Users\<you>\fvm\default\bin\flutter.bat)."
}

$FlutterBin = Resolve-FlutterBin -Preferred $FlutterBin

$psi = [System.Diagnostics.ProcessStartInfo]::new()
$flutterExt = [System.IO.Path]::GetExtension($FlutterBin).ToLowerInvariant()
$fvmCmd = Get-Command fvm -ErrorAction SilentlyContinue
if (($flutterExt -eq ".bat" -or $flutterExt -eq ".cmd") -and $null -ne $fvmCmd) {
    $psi.FileName = $fvmCmd.Source
    $psi.Arguments = "flutter run -d $FlutterDevice"
}
elseif ($flutterExt -eq ".bat" -or $flutterExt -eq ".cmd") {
    $escapedFlutterBin = $FlutterBin.Replace('"', '""')
    $psi.FileName = "cmd.exe"
    $psi.Arguments = "/d /c ""`"$escapedFlutterBin`" run -d $FlutterDevice"""
}
else {
    $psi.FileName = $FlutterBin
    $psi.Arguments = "run -d $FlutterDevice"
}
$psi.WorkingDirectory = $FlutterProjectDir
$psi.UseShellExecute = $false
$psi.RedirectStandardInput = $true
$psi.RedirectStandardOutput = $false
$psi.RedirectStandardError = $false
$psi.CreateNoWindow = $true
if (-not [string]::IsNullOrWhiteSpace($RustLog)) {
    $psi.EnvironmentVariables["RUST_LOG"] = $RustLog
}

$proc = [System.Diagnostics.Process]::new()
$proc.StartInfo = $psi

Write-Host "Starting flutter run..."
$null = $proc.Start()
Start-Sleep -Seconds $BootWaitSec
if ($proc.HasExited) {
    throw "flutter run exited during boot wait with code $($proc.ExitCode). See log: $logPath"
}

$samples = New-Object System.Collections.Generic.List[object]

for ($i = 1; $i -le $Iterations; $i++) {
    if ($proc.HasExited) {
        throw "flutter run exited early with code $($proc.ExitCode) at iteration $i"
    }

    $proc.StandardInput.WriteLine("R")
    $proc.StandardInput.Flush()

    Start-Sleep -Milliseconds $RestartIntervalMs

    $asioCount = @(
        Get-Process -Name "stellatune-asio-host" -ErrorAction SilentlyContinue
    ).Count
    $ringCount = 0
    if (Test-Path $RingDir) {
        $ringCount = @(
            Get-ChildItem -Path $RingDir -Filter "ring-*.shm" -ErrorAction SilentlyContinue
        ).Count
    }

    $samples.Add([PSCustomObject]@{
            TimestampUtc = (Get-Date).ToUniversalTime().ToString("o")
            Iteration = $i
            AsioHostProcessCount = $asioCount
            RingFileCount = $ringCount
        })

    Write-Host ("[{0}/{1}] asio_host={2} ring_files={3}" -f $i, $Iterations, $asioCount, $ringCount)
}

if (-not $KeepFlutterRunning) {
    if (-not $proc.HasExited) {
        $proc.StandardInput.WriteLine("q")
        $proc.StandardInput.Flush()
        $null = $proc.WaitForExit(15000)
    }
}

if (-not $proc.HasExited) {
    Write-Warning "flutter run is still alive; killing process."
    $proc.Kill($true)
    $null = $proc.WaitForExit(5000)
}

$samples | Export-Csv -Path $samplesPath -NoTypeInformation -Encoding UTF8

function Read-LogLines {
    param(
        [string]$Path,
        [int]$Retries = 1,
        [int]$RetryIntervalMs = 250
    )
    $lines = @()
    for ($attempt = 1; $attempt -le $Retries; $attempt++) {
        if (Test-Path $Path) {
            try {
                $lines = Get-Content -Path $Path -ErrorAction Stop
            }
            catch {
                $lines = @()
            }
        }
        if ($lines.Count -gt 0 -or $attempt -eq $Retries) {
            break
        }
        Start-Sleep -Milliseconds $RetryIntervalMs
    }
    return $lines
}

if (Test-Path $runtimeTracingLogPath) {
    # Give tracing writer a brief window to flush trailing records after app exit.
    Start-Sleep -Milliseconds 600
    Copy-Item -Path $runtimeTracingLogPath -Destination $logPath -Force
}
elseif (-not (Test-Path $logPath)) {
    Set-Content -Path $logPath -Value "" -Encoding UTF8
}

$logLines = Read-LogLines -Path $logPath -Retries 8 -RetryIntervalMs 300

function Get-MaxMetricFromLog {
    param(
        [string[]]$Lines,
        [string]$MetricName
    )
    if ($Lines.Count -eq 0) {
        return 0
    }

    $rawText = [string]::Join("`n", $Lines)
    $cleanText = $rawText -replace "$([char]27)\[[0-9;]*[A-Za-z]", ""
    $pattern = [regex]::new("$([regex]::Escape($MetricName))\s*=\s*([0-9]+)")
    $matches = $pattern.Matches($cleanText)
    if ($matches.Count -eq 0) {
        return 0
    }

    $max = 0
    foreach ($match in $matches) {
        $value = [int64]$match.Groups[1].Value
        if ($value -gt $max) {
            $max = $value
        }
    }
    return $max
}

$maxAsioHostProcessCount = 0
$maxRingFileCount = 0
if ($samples.Count -gt 0) {
    $maxAsioHostProcessCount = ($samples | Measure-Object -Property AsioHostProcessCount -Maximum).Maximum
    $maxRingFileCount = ($samples | Measure-Object -Property RingFileCount -Maximum).Maximum
}

$maxRuntimeHostInitsTotal = 0
$maxPlayerClientsActive = 0
$maxAsioSidecarSpawnsTotal = 0
$maxAsioSidecarRunning = 0
$maxPluginGenerationsDraining = 0

# Some tracing backends flush asynchronously after process exit.
# Poll for a short period and stop as soon as runtime_host_inits_total is parseable.
for ($attempt = 1; $attempt -le 30; $attempt++) {
    if (Test-Path $runtimeTracingLogPath) {
        Copy-Item -Path $runtimeTracingLogPath -Destination $logPath -Force
    }

    $logLines = Read-LogLines -Path $logPath -Retries 1 -RetryIntervalMs 100
    $maxRuntimeHostInitsTotal = Get-MaxMetricFromLog -Lines $logLines -MetricName "runtime_host_inits_total"
    if ($maxRuntimeHostInitsTotal -gt 0) {
        break
    }
    Start-Sleep -Milliseconds 300
}

$maxPlayerClientsActive = Get-MaxMetricFromLog -Lines $logLines -MetricName "player_clients_active"
$maxAsioSidecarSpawnsTotal = Get-MaxMetricFromLog -Lines $logLines -MetricName "asio_sidecar_spawns_total"
$maxAsioSidecarRunning = Get-MaxMetricFromLog -Lines $logLines -MetricName "asio_sidecar_running"
$maxPluginGenerationsDraining = Get-MaxMetricFromLog -Lines $logLines -MetricName "plugin_generations_draining"

$lostConnectionCount = @(
    $logLines | Select-String -Pattern "Lost connection to device|Lost Connection"
).Count

$summary = [PSCustomObject]@{
    Iterations = $Iterations
    RestartIntervalMs = $RestartIntervalMs
    FlutterBin = $FlutterBin
    RustLog = $RustLog
    FlutterProjectDir = $FlutterProjectDir
    FlutterExitCode = $proc.ExitCode
    LostConnectionCount = $lostConnectionCount
    MaxAsioHostProcessCount = $maxAsioHostProcessCount
    MaxRingFileCount = $maxRingFileCount
    MaxRuntimeHostInitsTotal = $maxRuntimeHostInitsTotal
    MaxPlayerClientsActive = $maxPlayerClientsActive
    MaxAsioSidecarSpawnsTotal = $maxAsioSidecarSpawnsTotal
    MaxAsioSidecarRunning = $maxAsioSidecarRunning
    MaxPluginGenerationsDraining = $maxPluginGenerationsDraining
    LogPath = $logPath
    SamplesPath = $samplesPath
}

$summary | ConvertTo-Json -Depth 4 | Set-Content -Path $summaryJsonPath -Encoding UTF8

@(
    "# Hot Restart Stress Summary"
    ""
    "- iterations: $($summary.Iterations)"
    "- restart_interval_ms: $($summary.RestartIntervalMs)"
    "- flutter_exit_code: $($summary.FlutterExitCode)"
    "- lost_connection_count: $($summary.LostConnectionCount)"
    "- max_asio_host_process_count: $($summary.MaxAsioHostProcessCount)"
    "- max_ring_file_count: $($summary.MaxRingFileCount)"
    "- max_runtime_host_inits_total: $($summary.MaxRuntimeHostInitsTotal)"
    "- max_player_clients_active: $($summary.MaxPlayerClientsActive)"
    "- max_asio_sidecar_spawns_total: $($summary.MaxAsioSidecarSpawnsTotal)"
    "- max_asio_sidecar_running: $($summary.MaxAsioSidecarRunning)"
    "- max_plugin_generations_draining: $($summary.MaxPluginGenerationsDraining)"
    ""
    "Artifacts:"
    "- $summaryJsonPath"
    "- $summaryMdPath"
    "- $samplesPath"
    "- $logPath"
) | Set-Content -Path $summaryMdPath -Encoding UTF8

Write-Host ""
Write-Host "Hot Restart stress run completed."
Write-Host "Summary:"
Write-Host "  $summaryJsonPath"
Write-Host "  $summaryMdPath"
