# VisiTexta

VisiTexta is a portable, offline desktop app that extracts text from images and PDFs into Markdown.

Version 0.2.0 uses a local GGUF vision model runtime (llama.cpp binaries) and does not require cloud APIs.

## What is new in 0.2.0
- Model-only OCR pipeline (no Tesseract in the processing path).
- Bundled llama.cpp runtime binaries for local inference.
- Model download flow in-app (Hugging Face) with automatic `mmproj` companion fetch for Qwen2-VL.
- Prompt input support (optional) for extraction behavior.
- Improved runtime checks and fallback to available vision models.

## Supported input
- PNG
- JPG / JPEG
- PDF

## Output
- Markdown (`.md`) written beside the source file.
- Live stream + preview in UI during processing.

## Portable quickstart (recommended)
1. Download release `0.2.0` from GitHub releases.
2. Unzip the package.
3. Run `VisiTexta.exe`.
4. Drop an image or PDF into the app.

No additional installation should be required if the release package includes:
- `bin/` (llama runtime binaries)
- `resources/`

Models are intentionally not bundled in the portable package to reduce release size.

## Expected portable layout (model-free release)
```text
VisiTexta 0.2.0/
  VisiTexta.exe
  bin/
  resources/
```

## Model requirements
VisiTexta expects a vision-capable GGUF model file name containing one of:
- `Vision`
- `-VL`
- `LLaVA`

For Qwen2.5-VL, include both files in `models/`:
- main model (example: `Qwen2.5-VL-3B-Instruct-Q3_K_S.gguf`)
- projector model (example: `mmproj-F16.gguf`)

If you download from Settings using a Qwen2-VL repo, VisiTexta will fetch the companion `mmproj` automatically.

## Development setup
From repo root:
```bash
cd app
npm install
npm run tauri:dev
```

## Build release (Windows)
From repo root:
```bash
cd app
npm run build
npm run tauri:build
```

Generated installers are under:
- `app/src-tauri/target/release/bundle/nsis/`
- `app/src-tauri/target/release/bundle/msi/`

## Packaging notes for GitHub release
When preparing a portable zip, copy:
- `app/src-tauri/target/release/app.exe` as `VisiTexta.exe`
- `app/src-tauri/bin/`
- `app/src-tauri/resources/`

## Troubleshooting
- `Missing model runtime`: ensure `bin/llama-mtmd-cli.exe` (or compatible llama runner) exists in package `bin/`.
- `no vision .gguf model found`: open Settings and download a vision model.
- Qwen2-VL startup errors: ensure `mmproj-*.gguf` exists in `models/`.

## Tech stack
- Frontend: React + TypeScript
- Desktop: Tauri
- Backend: Rust
- Model runtime: llama.cpp binaries
- PDF rendering: pdfium-render
