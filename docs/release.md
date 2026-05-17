# Release

Portable zip is the first packaging target.

Build:

```powershell
.\scripts\build-release.ps1 -Version 3.0.1
```

The release script publishes:

- `src/App.Desktop/App.Desktop.csproj`
- `workers/ocr-worker/OcrWorker.csproj`

It stages:

```text
VisiTexta-v<version>-win-x64/
  VisiTexta.exe
  workers/
    ocr-worker/
      ocr-worker.exe
  bin/
  resources/
  portable-data/
```

When a code-signing certificate is available, set these environment variables or pass the matching parameters:

```powershell
$env:VISITEXTA_CERT_PATH="C:\certs\VisiTexta.pfx"
$env:VISITEXTA_CERT_PASSWORD="<password>"
.\scripts\build-release.ps1 -Version 3.0.1
```

The script signs staged `.exe` files before zipping when a certificate is provided. Windows Smart App Control can still block unsigned portable builds, so public releases should use a trusted certificate and timestamp. Installer packaging is intentionally deferred until signing strategy is decided.
