# VisiTexta

VisiTexta is a portable, offline desktop OCR app that converts images and PDFs into clean Markdown. It runs fully on your machine with no cloud processing, and provides a simple drag-and-drop workflow plus live progress and preview.

## Highlights
- Offline OCR to Markdown.
- Supports PDF, PNG, JPG, JPEG.
- Drag-and-drop upload and file queue with progress.
- Markdown preview, copy, and save.
- Portable Windows exe (no installer required).
- Model manager with Hugging Face download and local model selection.

## How it works
1. PDFs are rendered to images (pdfium).
2. Tesseract OCR extracts text from each page.
3. Text is cleaned into Markdown (rule-based cleanup today; LLM cleanup can be added later).
4. A .md file is saved next to the source file.

## Quickstart (portable)
1. Download or build the portable folder.
2. Open Settings and download a model, or place a GGUF file into `models/`.
3. Run `VisiTexta.exe`.
4. Drop a PDF or image into the app.

Default model name used by the app if none is selected: `Qwen_Qwen3.5-0.8B-Q4_K_M.gguf`.

Portable folder layout:
```
VisiTexta/
  VisiTexta.exe
  bin/           # pdfium + tesseract deps
  resources/     # tessdata
  models/        # gguf models (not shipped)
```

## Model recommendations
For first-time use on low computing power:
- `unsloth/Qwen3.5-0.8B-GGUF`

For mid-range machines:
- `bartowski/Qwen_Qwen3.5-4B-GGUF`

For high-end machines:
- `Qwen/Qwen3-8B-GGUF`
- `Qwen/Qwen3-32B-GGUF`

Notes:
- Larger models require more RAM and will run slower on weaker CPUs.
- You can download models from Settings by entering the repo name, or place a `.gguf` file directly into `models/`.

## Model management
- Open Settings to select a local model or download one from Hugging Face by repo name, for example:
  - `Qwen/Qwen3.5-0.8B`
  - Or a specific file: `Qwen/Qwen3.5-0.8B/Qwen_Qwen3.5-0.8B-Q4_K_M.gguf`
- Download progress is shown in the Settings panel.
- Models are saved to `models/` next to the app.

Note: downloading a model requires an internet connection. OCR and formatting run fully offline.

## Development setup
From the repo root:
```
cd app
npm install
npm run tauri:dev
```

## Build (Windows)
From the repo root:
```
cd app
npm run build
npm run tauri:build
```

The NSIS installer is produced under:
`app/src-tauri/target/release/bundle/nsis/`

To create a portable folder, copy:
- `app/src-tauri/target/release/app.exe` as `VisiTexta.exe`
- `app/src-tauri/bin/`
- `app/src-tauri/resources/`
- `app/models/`

## Troubleshooting
- If the UI shows "Model missing", place a GGUF model in `models/` or download it via Settings.
- If OCR fails, confirm `pdfium.dll`, `tesseract.exe`, `libtesseract-5.dll`, and `libleptonica-6.dll` are in `bin/`.
- If drag and drop does not work, try the Browse button and confirm the file extension is supported.

## Tech stack
- Frontend: React + TypeScript
- Desktop: Tauri
- Backend: Rust
- OCR: Tesseract
- PDF rendering: pdfium-render
- Model runtime: llama.cpp (planned for advanced cleanup)
