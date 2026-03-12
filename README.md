# VisiTexta

VisiTexta is a cross-platform OCR studio built with Tauri + React. It runs vision-language models locally, downloads them on first use, and keeps them cached.

## Features

- Image and PDF OCR (page-by-page)
- Progressive output with real-time status updates
- Model selector (minimum/medium/high + custom HuggingFace IDs)
- Local model cache with first-time download progress
- Merge PDF pages into a single article
- Export results to TXT/Markdown
- Modern UI with animated scanning effects

## Tech Stack

- **Frontend**: React + Tailwind (Vite)
- **Desktop**: Tauri v2
- **Backend**: Rust (model management, GPU detection, PDF handling, inference)

## Getting Started

```bash
npm install
npm run dev
```

## Build Desktop App

```bash
npm run build
npm run tauri build
```

## Notes

- The Rust backend currently ships placeholder OCR output. Replace `process_image`, `process_pdf`, and `merge_pages` in `src-tauri/src/main.rs` with the real inference pipeline.
- Model downloads are stubbed in `ensure_model` and should be wired to your local runtime (e.g. `llama.cpp` or `transformers` with a local runner).
- PDF preview uses `pdfjs-dist` inside the UI, while the backend should handle page rasterization for OCR.

## Roadmap

- Implement real model download + caching
- GPU detection on Windows/macOS (CUDA/Metal)
- PDF rasterization and page OCR
- Memory limit enforcement for model loading
- Optional iOS build (shared logic + mobile UI shell)

