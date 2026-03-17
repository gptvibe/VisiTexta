# VisiTexta App

Desktop app package for VisiTexta 0.2.0 (Tauri + React + Rust).

## Runtime architecture (0.2.0)
- Model-only OCR path via local llama runtime binaries.
- Vision GGUF model required in `models/`.
- For Qwen2.5-VL models, a matching `mmproj` file is required.

## Run in development
```bash
npm install
npm run tauri:dev
```

## Build
```bash
npm run build
npm run tauri:build
```

## Required local files
- `src-tauri/bin/`:
  - `llama-mtmd-cli.exe` (or compatible runner)
  - required llama/ggml DLLs
- `models/` is created automatically when the user downloads a model from Settings.

## Notes
- Inputs: PDF, PNG, JPG, JPEG.
- Output: markdown file beside input source.
- Processing is fully offline once runtime/model files are present.
- For Qwen2-VL repo downloads, companion `mmproj` is downloaded automatically.
