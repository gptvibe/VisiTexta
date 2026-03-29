$ErrorActionPreference = 'Stop'

$appRoot = Split-Path -Parent $PSScriptRoot
$tauriManifest = Join-Path $appRoot 'src-tauri\Cargo.toml'

Write-Host "Running release QA build checks..."
Push-Location $appRoot
try {
  npm run build
  cargo check --manifest-path $tauriManifest

  Write-Host "Running cold-start benchmark gate..."
  npm run benchmark:gate:cold

  Write-Host "Running warm-start benchmark gate..."
  npm run benchmark:gate:warm
} finally {
  Pop-Location
}

Write-Host "Release QA passed."
