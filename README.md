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

- If no supported OCR model is found, VisiTexta will start downloading the recommended default profile automatically.
- The default profile is GLM-OCR using `GLM-OCR.Q4_K_M.gguf`.
- This is normal and only happens on first setup (or if you removed supported models).
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

- VisiTexta now uses an explicit curated model registry instead of treating arbitrary GGUF filenames as fully supported.
- GLM-OCR is the recommended default profile.
- Additional curated profiles include Qwen2-VL OCR 2B and Qwen2.5-VL 3B.
- Some curated models also need an `mmproj` file. If required, VisiTexta validates the download and fetches the companion `mmproj` automatically.
- Advanced settings still include an experimental custom download field for power users, but unlisted GGUF models are treated as best-effort only.
- For experimental custom downloads, enter a full `owner/repo/file.gguf` path. Repo-only auto-selection is reserved for the curated supported profiles.

## Troubleshooting

- Error about missing runtime CLI:
  Make sure `bin/llama-mtmd-cli.exe` and `bin/llama-cli.exe` exist.
- Error about missing model:
  Open Settings and download one of the curated profiles (or let the GLM-OCR auto-download finish).
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
