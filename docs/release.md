# Release

Portable zip is the first packaging target.

Build:

```powershell
.\scripts\build-release.ps1
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

Installer packaging is intentionally deferred until signing strategy is decided.
