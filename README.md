# VisiTexta

VisiTexta is a Windows desktop app that extracts text from images and PDFs and saves the result as Markdown.

It runs locally on your PC. No cloud OCR API is required.

## Who is this for?

- Students who want text from notes or scanned pages.
- Office users who need text from screenshots or PDFs.
- Anyone who wants simple OCR output in `.md` format.

## What is new in 1.0.0

- Works reliably for both images and PDFs.
- Streams OCR text live in the app while processing.
- Auto-downloads the default model on first run if no model is installed.
- Uses a local runtime bundle in the release package.
- Produces cleaner OCR-first Markdown output.

## Supported files

- PNG
- JPG / JPEG
- PDF

## What you get

- A Markdown file (`.md`) saved next to your original file.
- Live preview in the app while OCR runs.

## Quick start (for normal users)

1. Download release `1.0.0`.
2. Unzip it.
3. Open `VisiTexta.exe`.
4. Drop an image or PDF into the app.

### First run behavior (important)

- If no model is found, VisiTexta will start downloading the default model automatically.
- This is normal and only happens on first setup (or if you removed models).
- Keep the app open until the download completes.

### Why first output can feel slow

- The first word may take a while to appear.
- On the first page, the model is loading and preparing context.
- After that, output streams progressively.

In short: initial delay is expected, then text should start flowing.

## Portable package layout

```text
VisiTexta 1.0.0/
  VisiTexta.exe
  bin/
  resources/
  models/
```

## Model notes

- Supported vision models include GLM-OCR, Qwen-VL, and similar vision GGUF files.
- Some models also need an `mmproj` file.
- If required, VisiTexta downloads the companion `mmproj` automatically during model download.

## Troubleshooting

- Error about missing runtime CLI:
  Make sure `bin/llama-mtmd-cli.exe` and `bin/llama-cli.exe` exist.
- Error about missing model:
  Open Settings and download a model (or let auto-download finish).
- Error about missing `mmproj`:
  Re-run model download from Settings so companion files are fetched.

## For developers

From repo root:

```bash
cd app
npm install
npm run tauri:dev
```

Build release:

```bash
cd app
npm run build
npm run tauri:build
```
