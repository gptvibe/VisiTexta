$ErrorActionPreference = 'Stop'

$appRoot = Split-Path -Parent $PSScriptRoot
$tauriRoot = Join-Path $appRoot 'src-tauri'
$buildTarget = $env:CARGO_BUILD_TARGET
$releaseRoots = @()
if ($buildTarget) {
  $releaseRoots += Join-Path $tauriRoot ("target\" + $buildTarget + "\release")
}
$releaseRoots += Join-Path $tauriRoot 'target\release'
$configPath = Join-Path $tauriRoot 'tauri.conf.json'
$config = Get-Content -Raw $configPath | ConvertFrom-Json
$productName = $config.productName

Write-Host "Building VisiTexta release binary..."
Push-Location $appRoot
try {
  npm exec tauri build -- --no-bundle
} finally {
  Pop-Location
}

$releaseRoot = $releaseRoots |
  Where-Object { Test-Path $_ } |
  Select-Object -First 1

if (-not $releaseRoot) {
  throw "Could not find a release output folder. Checked: $($releaseRoots -join ', ')"
}

$portableRoot = Join-Path $releaseRoot 'portable'

$exeCandidates = Get-ChildItem $releaseRoot -Filter *.exe |
  Where-Object { $_.Name -notmatch 'setup|installer|uninstall|updater|ocr_bench' }

$exe = $exeCandidates |
  Where-Object { $_.BaseName -in @('app', $productName) } |
  Sort-Object LastWriteTime -Descending |
  Select-Object -First 1

if (-not $exe) {
  $exe = $exeCandidates |
    Sort-Object LastWriteTime -Descending |
    Select-Object -First 1
}

if (-not $exe) {
  throw "Could not find the built release executable in $releaseRoot"
}

$stageDir = Join-Path $portableRoot $productName
$stageExe = Join-Path $stageDir "$productName.exe"
$portableDataDir = Join-Path $stageDir 'portable-data'
$zipPath = Join-Path $portableRoot "$productName-portable.zip"

if (Test-Path $stageDir) {
  Remove-Item -LiteralPath $stageDir -Recurse -Force
}
if (Test-Path $zipPath) {
  Remove-Item -LiteralPath $zipPath -Force
}

New-Item -ItemType Directory -Path $stageDir | Out-Null
New-Item -ItemType Directory -Path $portableDataDir | Out-Null
Set-Content -LiteralPath (Join-Path $portableDataDir '.keep') -Value 'VisiTexta portable data lives here.' -Encoding ascii

Copy-Item -LiteralPath $exe.FullName -Destination $stageExe

foreach ($folder in @('bin', 'resources')) {
  $source = Join-Path $tauriRoot $folder
  if (Test-Path $source) {
    Copy-Item -LiteralPath $source -Destination (Join-Path $stageDir $folder) -Recurse
  }
}

Compress-Archive -Path (Join-Path $stageDir '*') -DestinationPath $zipPath

Write-Host "Portable package staged at $stageDir"
Write-Host "Portable zip created at $zipPath"
