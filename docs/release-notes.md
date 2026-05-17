# Release Notes

## 2.0.1

VisiTexta 2.0.1 is a focused patch release for the Tauri desktop app.

Highlights:

- New OCR runs now switch the workspace to the live OCR pane immediately, so streaming text is visible while extraction is still in progress.
- Starting a new job now clears the previous selection first, allowing the first incoming stream event to claim the preview instead of waiting for the final invoke result.
- Packaging metadata is updated for the 2.0.1 release.

## 2.0.0

VisiTexta 2.0.0 is a local-first Windows OCR desktop app for images and PDFs. It ships with a bundled local runtime path, supports first-run model download when no supported model is already installed, and keeps OCR output next to the original source file instead of hiding it inside app storage.

Highlights:

- First-run setup now auto-downloads the recommended GLM-OCR profile when no curated OCR model is available yet.
- Storage behavior is explicit. Installer builds use `%LOCALAPPDATA%\VisiTexta\`, while portable builds keep settings, history, models, temp files, and pasted inputs in `portable-data\` beside the executable.
- Runtime profiles are now clearer:
  `CPU compatible` is the widest-compatibility default.
  `Auto` prefers a compatible accelerated runtime when one is bundled.
  `Accelerated if available` tries the accelerated runtime first, then falls back cleanly when needed.
- The preview workspace is calmer on laptop screens. Jobs now use segmented views for Original, OCR, Notes / Extract, and Export, with a compact status bar for model, runtime, storage mode, and progress.
- Notes workflow adds source-linked page references, plus Markdown, plain text, and searchable text-based PDF export.
- Extract workflow adds invoice / receipt, table-to-CSV, meeting photo / whiteboard, and contract key-point presets. Presets return readable Markdown plus structured JSON, and CSV where it applies. Extract output also includes an uncertainty / verification section for fields that may need manual review.
- Release QA now includes benchmark gating. Warm and cold OCR benchmark baselines live under `app/benchmarks/baselines/`, and `npm --prefix app run release:qa` runs build checks plus regression gates for time-to-first-text, total runtime, and peak memory.

Packaging notes:

- `npm --prefix app run tauri:build` now uses the gated release path.
- `npm --prefix app run tauri:build:installer` and `npm --prefix app run tauri:build:portable` also run release QA before packaging.
- `unsafe` script variants remain available for local packaging work when you explicitly want to bypass the release gate.
