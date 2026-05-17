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

Write-Host "Running release QA build checks..."
Push-Location $appRoot
try {
  Invoke-CheckedCommand { npm run build } 'Frontend build failed'
  Invoke-CheckedCommand { cargo check --manifest-path $tauriManifest } 'Cargo check failed'

  Write-Host "Running cold-start benchmark gate..."
  Invoke-CheckedCommand { npm run benchmark:gate:cold } 'Cold benchmark gate failed'

  Write-Host "Running warm-start benchmark gate..."
  Invoke-CheckedCommand { npm run benchmark:gate:warm } 'Warm benchmark gate failed'
} finally {
  Pop-Location
}

Write-Host "Release QA passed."
