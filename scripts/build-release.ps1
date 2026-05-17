param(
    [string]$Version = "3.0.2",
    [string]$SignToolPath = $env:VISITEXTA_SIGNTOOL,
    [string]$CertificatePath = $env:VISITEXTA_CERT_PATH,
    [string]$CertificatePassword = $env:VISITEXTA_CERT_PASSWORD,
    [string]$TimestampUrl = $(if ($env:VISITEXTA_TIMESTAMP_URL) { $env:VISITEXTA_TIMESTAMP_URL } else { "http://timestamp.digicert.com" })
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

function Copy-AssetFolder([string]$Name) {
    $candidates = @(
        (Join-Path $repoRoot $Name),
        (Join-Path $repoRoot "app\src-tauri\$Name")
    )

    $source = $candidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
    if ($null -eq $source) {
        return
    }

    $destination = Join-Path $stageDir $Name
    Remove-DirectoryIfExists $destination
    Copy-Item -LiteralPath $source -Destination $destination -Recurse -Force
}

function Copy-XamlBuildArtifacts([string]$DestinationRoot) {
    $desktopBuildRoot = Join-Path $repoRoot "src\App.Desktop\bin\Release"
    $priFile = Get-ChildItem -LiteralPath $desktopBuildRoot -Recurse -Filter "VisiTexta.pri" -File -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending |
        Select-Object -First 1

    if ($null -eq $priFile) {
        throw "WinUI resource file VisiTexta.pri was not found in the Release build output. The portable app would crash without it."
    }

    $xamlRoot = Split-Path -Parent $priFile.FullName
    $xamlFiles = Get-ChildItem -LiteralPath $xamlRoot -Recurse -Include *.xbf,*.pri -File
    foreach ($file in $xamlFiles) {
        $relative = [System.IO.Path]::GetRelativePath($xamlRoot, $file.FullName)
        $destination = Join-Path $DestinationRoot $relative
        $destinationDirectory = Split-Path -Parent $destination
        New-Item -ItemType Directory -Force -Path $destinationDirectory | Out-Null
        Copy-Item -LiteralPath $file.FullName -Destination $destination -Force
    }
}

function Resolve-SignToolPath {
    if (-not [string]::IsNullOrWhiteSpace($SignToolPath) -and (Test-Path -LiteralPath $SignToolPath)) {
        return $SignToolPath
    }

    $windowsKitsRoot = Join-Path ${env:ProgramFiles(x86)} "Windows Kits\10\bin"
    if (-not (Test-Path -LiteralPath $windowsKitsRoot)) {
        return $null
    }

    return Get-ChildItem -LiteralPath $windowsKitsRoot -Recurse -Filter signtool.exe -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -like "*\x64\signtool.exe" } |
        Sort-Object FullName -Descending |
        Select-Object -First 1 -ExpandProperty FullName
}

function Invoke-OptionalCodeSigning {
    if ([string]::IsNullOrWhiteSpace($CertificatePath)) {
        Write-Host "Code signing skipped. Set VISITEXTA_CERT_PATH or pass -CertificatePath to sign the portable release."
        return
    }

    if (-not (Test-Path -LiteralPath $CertificatePath)) {
        throw "Certificate file was not found: $CertificatePath"
    }

    $resolvedSignTool = Resolve-SignToolPath
    if ([string]::IsNullOrWhiteSpace($resolvedSignTool)) {
        throw "signtool.exe was not found. Pass -SignToolPath or install the Windows SDK signing tools."
    }

    $signArgs = @("sign", "/fd", "SHA256", "/f", $CertificatePath, "/tr", $TimestampUrl, "/td", "SHA256")
    if (-not [string]::IsNullOrWhiteSpace($CertificatePassword)) {
        $signArgs += @("/p", $CertificatePassword)
    }

    $targets = Get-ChildItem -LiteralPath $stageDir -Recurse -Include *.exe -File |
        Where-Object { $_.FullName -notlike "*\portable-data\*" }

    foreach ($target in $targets) {
        & $resolvedSignTool @signArgs $target.FullName
        if ($LASTEXITCODE -ne 0) {
            throw "Code signing failed for $($target.FullName)"
        }
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
Copy-XamlBuildArtifacts $stageDir

$workerStageDir = Join-Path $stageDir "workers\ocr-worker"
Remove-DirectoryIfExists $workerStageDir
New-Item -ItemType Directory -Force -Path $workerStageDir | Out-Null
Copy-Item -Path (Join-Path $workerPublishDir "*") -Destination $workerStageDir -Recurse -Force

Copy-AssetFolder "bin"
Copy-AssetFolder "resources"

$portableDataDir = Join-Path $stageDir "portable-data"
New-Item -ItemType Directory -Force -Path $portableDataDir | Out-Null
Set-Content -LiteralPath (Join-Path $portableDataDir ".keep") -Value "VisiTexta portable data lives here." -Encoding ascii

Invoke-OptionalCodeSigning

Compress-Archive -Path (Join-Path $stageDir "*") -DestinationPath $portableZip -Force
Get-Item -LiteralPath $portableZip | Select-Object FullName, Length
