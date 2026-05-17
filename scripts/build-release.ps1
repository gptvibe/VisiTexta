param(
    [string]$Version = "0.3.0"
)

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$publishDir = Join-Path $repoRoot "artifacts\publish\VisiTexta-win-x64"
$workerPublishDir = Join-Path $repoRoot "artifacts\publish\ocr-worker-win-x64"
$releaseDir = Join-Path $repoRoot "artifacts\release"
$stageDir = Join-Path $releaseDir "VisiTexta-v$Version-win-x64"
$portableZip = Join-Path $releaseDir "VisiTexta-v$Version-win-x64-portable.zip"

function Remove-DirectoryIfExists([string]$Path) {
    if (Test-Path -LiteralPath $Path) {
        Remove-Item -LiteralPath $Path -Recurse -Force
    }
}

New-Item -ItemType Directory -Force -Path $releaseDir | Out-Null
Remove-DirectoryIfExists $publishDir
Remove-DirectoryIfExists $workerPublishDir
Remove-DirectoryIfExists $stageDir
if (Test-Path -LiteralPath $portableZip) {
    Remove-Item -LiteralPath $portableZip -Force
}

dotnet publish (Join-Path $repoRoot "src\App.Desktop\App.Desktop.csproj") `
    -c Release `
    -r win-x64 `
    -p:WindowsPackageType=None `
    -p:SelfContained=true `
    -p:PublishSingleFile=false `
    -p:PublishReadyToRun=false `
    -p:PublishTrimmed=false `
    -p:Version=$Version `
    -o $publishDir

dotnet publish (Join-Path $repoRoot "workers\ocr-worker\OcrWorker.csproj") `
    -c Release `
    -r win-x64 `
    -p:SelfContained=true `
    -p:PublishSingleFile=false `
    -p:PublishTrimmed=false `
    -p:Version=$Version `
    -o $workerPublishDir

Copy-Item -LiteralPath $publishDir -Destination $stageDir -Recurse

$workerStageDir = Join-Path $stageDir "workers\ocr-worker"
Remove-DirectoryIfExists $workerStageDir
New-Item -ItemType Directory -Force -Path $workerStageDir | Out-Null
Copy-Item -Path (Join-Path $workerPublishDir "*") -Destination $workerStageDir -Recurse -Force

foreach ($folder in @("bin", "resources")) {
    $source = Join-Path $repoRoot $folder
    if (Test-Path -LiteralPath $source) {
        Copy-Item -LiteralPath $source -Destination (Join-Path $stageDir $folder) -Recurse -Force
    }
}

$portableDataDir = Join-Path $stageDir "portable-data"
New-Item -ItemType Directory -Force -Path $portableDataDir | Out-Null
Set-Content -LiteralPath (Join-Path $portableDataDir ".keep") -Value "VisiTexta portable data lives here." -Encoding ascii

Compress-Archive -Path (Join-Path $stageDir "*") -DestinationPath $portableZip -Force
Get-Item -LiteralPath $portableZip | Select-Object FullName, Length
