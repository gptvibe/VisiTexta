# VisiTexta

Offline OCR-to-Markdown desktop app (Tauri + React + Rust).

## Quickstart (dev)
1. Ensure the model file exists at `models/Qwen_Qwen3.5-0.8B-Q4_K_M.gguf` (already in this repo root).
2. DLLs (pdfium + tesseract) are under `src-tauri/bin`; tessdata (eng, osd) under `src-tauri/resources/tessdata`.
3. Run:
   ```bash
   npm install
   npm run tauri:dev
   ```
   Drop a PDF or image; a `<basename>.md` file is written beside the source.

## Build portable exe
```bash
npm run build
npm run tauri:build
```
The bundle includes `bin/*` and `resources/tessdata`.

## Features
- Inputs: PNG, JPG, JPEG, PDF (PDF rendered via pdfium to images).
- OCR: Tesseract (eng by default; add languages to `src-tauri/resources/tessdata` or `models/tessdata`).
- Output: cleaned Markdown per page; copies available via UI buttons.
- Offline: all processing local; model loaded from `./models`.

## Troubleshooting
- If the UI shows "Model missing", place the model file in `models/`.
- If OCR fails, confirm `pdfium.dll`, `libtesseract-5.dll`, `libleptonica-6.dll` are present in `src-tauri/bin`.
