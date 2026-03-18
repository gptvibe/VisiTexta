# VisiTexta App

Desktop app package for VisiTexta 1.0.0 (Tauri + React + Rust).

## Runtime architecture (1.0.0)
- Model-only OCR path via local llama runtime binaries.
- Vision GGUF model required in `models/`.
- For GLM-OCR, Qwen-VL, and LLaVA-family models, a matching `mmproj` file is required.
- OCR work runs off the command handler path to keep UI responsiveness under load.
- OCR text is streamed progressively into the preview while processing each page.
- PDFs are rendered locally through PDFium before page-by-page OCR.

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
- For supported multimodal repo downloads, companion `mmproj` is downloaded automatically when needed.
