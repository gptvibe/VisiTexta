$ErrorActionPreference = 'Stop'

$appRoot = Split-Path -Parent $PSScriptRoot
$tauriManifest = Join-Path $appRoot 'src-tauri\Cargo.toml'

function Invoke-CheckedCommand {
  param(
    [Parameter(Mandatory = $true)]
    [scriptblock]$Command,
    [Parameter(Mandatory = $true)]
    [string]$FailureMessage
  )

  & $Command
  if ($LASTEXITCODE -ne 0) {
    throw "$FailureMessage (exit code $LASTEXITCODE)"
  }
}

function Invoke-CapturedCommand {
  param(
    [Parameter(Mandatory = $true)]
    [string]$FilePath,
    [Parameter()]
    [string]$Arguments = '',
    [Parameter()]
    [string]$WorkingDirectory = $appRoot
  )

  $startInfo = New-Object System.Diagnostics.ProcessStartInfo
  $startInfo.FileName = $FilePath
  $startInfo.WorkingDirectory = $WorkingDirectory
  $startInfo.UseShellExecute = $false
  $startInfo.CreateNoWindow = $true
  $startInfo.RedirectStandardOutput = $true
  $startInfo.RedirectStandardError = $true
  $startInfo.Arguments = $Arguments

  $process = New-Object System.Diagnostics.Process
  $process.StartInfo = $startInfo
  [void]$process.Start()

  $stdout = $process.StandardOutput.ReadToEnd()
  $stderr = $process.StandardError.ReadToEnd()
  $process.WaitForExit()

  if ($stdout.Length -gt 0) {
    Write-Host ($stdout.TrimEnd())
  }

  if ($stderr.Length -gt 0) {
    Write-Host ($stderr.TrimEnd())
  }

  return [pscustomobject]@{
    ExitCode = $process.ExitCode
    OutputText = ($stdout + [Environment]::NewLine + $stderr)
  }
}

function Stop-ReleaseBlockingProcesses {
  $names = @('app', 'VisiTexta', 'ocr_bench', 'llama-server', 'llama-mtmd-cli')
  foreach ($name in $names) {
    $running = Get-Process -Name $name -ErrorAction SilentlyContinue
    if ($running) {
      $running | Stop-Process -Force -ErrorAction Stop
      Write-Host "Stopped running process '$name' to avoid release file locks."
    }
  }
}

function Test-LlamaServerBlockedByPolicy {
  $runnerPath = Join-Path $appRoot 'src-tauri\bin\llama-server.exe'
  if (-not (Test-Path -LiteralPath $runnerPath)) {
    return $false
  }

  $previousNativePreference = $PSNativeCommandUseErrorActionPreference
  $PSNativeCommandUseErrorActionPreference = $false
  try {
    try {
      & $runnerPath '--help' *> $null
      return $false
    } catch {
      return (($_ | Out-String) -match 'Application Control policy has blocked this file')
    }
  } finally {
    $PSNativeCommandUseErrorActionPreference = $previousNativePreference
  }
}

function Test-TransientBenchmarkCrash {
  param(
    [Parameter(Mandatory = $true)]
    [pscustomobject]$Result
  )

  return $Result.ExitCode -eq -1
}

Write-Host "Running release QA build checks..."
Push-Location $appRoot
try {
  Stop-ReleaseBlockingProcesses
  Invoke-CheckedCommand { npm run build } 'Frontend build failed'
  Invoke-CheckedCommand { cargo check --manifest-path $tauriManifest } 'Cargo check failed'

  Write-Host "Running cold-start benchmark gate..."
  $coldAttempt = 1
  $benchmarkAttemptLimit = 2
  while ($true) {
    $coldResult = Invoke-CapturedCommand -FilePath $env:ComSpec -Arguments '/d /c "npm run benchmark:gate:cold"'
    if ($coldResult.ExitCode -eq 0) {
      break
    }

    if ((Test-TransientBenchmarkCrash -Result $coldResult) -and $coldAttempt -lt $benchmarkAttemptLimit) {
      Write-Warning 'Cold benchmark gate crashed with exit code -1. Retrying once after cleaning up OCR worker processes.'
      Stop-ReleaseBlockingProcesses
      $coldAttempt += 1
      continue
    }

    throw "Cold benchmark gate failed (exit code $($coldResult.ExitCode))"
  }

  Write-Host "Running warm-start benchmark gate..."
  $warmAttempt = 1
  while ($true) {
    $warmResult = Invoke-CapturedCommand -FilePath $env:ComSpec -Arguments '/d /c "npm run benchmark:gate:warm"'
    if ($warmResult.ExitCode -eq 0) {
      break
    }

    if (Test-LlamaServerBlockedByPolicy) {
      Write-Warning 'Warm benchmark gate skipped because Windows Application Control blocked llama-server.exe on this machine.'
      break
    }

    if ((Test-TransientBenchmarkCrash -Result $warmResult) -and $warmAttempt -lt $benchmarkAttemptLimit) {
      Write-Warning 'Warm benchmark gate crashed with exit code -1. Retrying once after cleaning up OCR worker processes.'
      Stop-ReleaseBlockingProcesses
      $warmAttempt += 1
      continue
    }

    throw "Warm benchmark gate failed (exit code $($warmResult.ExitCode))"
  }
} finally {
  Pop-Location
}

Write-Host "Release QA passed."
