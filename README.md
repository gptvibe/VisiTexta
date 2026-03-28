# VisiTexta

VisiTexta is a Windows desktop app that extracts text from images and PDFs and saves the result as Markdown.

It runs locally on your PC. No cloud OCR API is required.

This repo currently targets Windows behavior explicitly.

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

- A Markdown file saved next to your original file as `file.ocr.md`.
- If that file name already exists, VisiTexta saves `file (ocr 2).md`, `file (ocr 3).md`, and so on instead of overwriting anything.
- Live preview in the app while OCR runs.

## Quick start (for normal users)

1. Download release `1.0.0`.
2. Choose one package style:
3. For portable use, unzip the app and run `VisiTexta.exe`.
4. For installer use, run the Windows installer and launch VisiTexta from the installed app.
5. Drop an image or PDF into the app.

## Portable vs installer behavior

### Portable mode

- Portable mode is intended for an unpacked copy of the app.
- VisiTexta stores its own app data beside the executable in `portable-data\`.
- That includes:
  `portable-data\settings.json`
  `portable-data\history.json`
  `portable-data\models\`
  `portable-data\temp\`
  `portable-data\pasted-inputs\`
- No OS config directory is used while portable mode is active.
- Portable mode is selected automatically for unpacked copies outside common Windows install folders.
- You can also force portable mode by putting `portable-data\` or `visitexta-portable.txt` beside `VisiTexta.exe` before first launch.

### Installer mode

- Installer mode is intended for the normal Windows-installed app.
- VisiTexta stores settings, history, models, temp files, and pasted inputs under:
  `%LOCALAPPDATA%\VisiTexta\`
- This keeps the install folder clean and matches normal Windows app expectations.

### What users see in the app

- Settings now shows the exact storage mode and the exact paths for settings, history, models, and temp files.
- OCR output files are still written next to the source file, not inside the app-data folder.

### First run behavior (important)

- If no supported OCR model is found, VisiTexta will start downloading the recommended default profile automatically.
- The default profile is GLM-OCR using `GLM-OCR.Q4_K_M.gguf`.
- This is normal and only happens on first setup (or if you removed supported models).
- Keep the app open until the download completes.
- Curated model downloads resume from existing partial `.part` files when Hugging Face supports ranged downloads.
- Curated model downloads are checksum-verified before they are accepted.

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
  portable-data/
```

## Model notes

- VisiTexta now uses an explicit curated model registry instead of treating arbitrary GGUF filenames as fully supported.
- GLM-OCR is the recommended default profile.
- Additional curated profiles include Qwen2-VL OCR 2B and Qwen2.5-VL 3B.
- Some curated models also need an `mmproj` file. If required, VisiTexta validates the download and fetches the companion `mmproj` automatically.
- Existing legacy model folders are still discovered during upgrades so older installs do not break abruptly.
- New downloads always go to the active primary storage location shown in Settings.
- Advanced settings still include an experimental custom download field for power users, but unlisted GGUF models are treated as best-effort only.
- For experimental custom downloads, enter a full `owner/repo/file.gguf` path. Repo-only auto-selection is reserved for the curated supported profiles.

## Temp files and recovery

- Temporary OCR work files are kept in the app-managed temp folder and are cleaned on startup.
- If VisiTexta closes during a job, the interrupted job is kept in history and marked as failed on the next launch.
- Pasted images are stored in the active app-data location so retries and history stay predictable.

## Troubleshooting

- Error about missing runtime CLI:
  Make sure `bin/llama-mtmd-cli.exe` and `bin/llama-cli.exe` exist.
- Error about missing model:
  Open Settings and download one of the curated profiles (or let the GLM-OCR auto-download finish).
- Error about missing `mmproj`:
  Re-run model download from Settings so companion files are fetched.
- Portable copy is using `%LOCALAPPDATA%` when you expected portable mode:
  Put `portable-data\` or `visitexta-portable.txt` beside `VisiTexta.exe`, then launch it again.

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

Release notes for packagers:

- Portable packages should include a sibling `portable-data\` folder or `visitexta-portable.txt` marker so the mode is unambiguous even before first run.
- Installer packages should be installed normally; app data lives under `%LOCALAPPDATA%\VisiTexta`, not in the install directory.
- `npm run tauri:build:installer` builds the Windows installer bundles.
- `npm run tauri:build:portable` builds a no-bundle release executable, stages `portable-data\`, and creates a portable zip.
